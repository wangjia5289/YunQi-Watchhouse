use std::{
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    time::Duration,
};

use objc2::{AnyThread, rc::autoreleasepool};
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
use objc2_core_graphics::{CGEventSource, CGEventSourceStateID, CGEventType};
use objc2_foundation::{NSDictionary, NSString};

use crate::{
    error::{AppError, AppResult},
    platform::{ForegroundApplication, ForegroundApplicationProvider, IdleTimeProvider},
};

/// Reads macOS' aggregate HID idle duration through the public CoreGraphics API.
///
/// `kCGAnyInputEventType` is intentionally represented as `u32::MAX`: Apple
/// defines it as an enum sentinel, while objc2 skips generating that constant.
/// This call queries elapsed time only and does not install an event tap.
#[derive(Debug, Default, Clone, Copy)]
pub struct MacOsActivityProvider;

pub fn application_icon_png(executable_path: &str) -> AppResult<Vec<u8>> {
    let icon_path = application_bundle_path(Path::new(executable_path))
        .unwrap_or_else(|| PathBuf::from(executable_path));

    autoreleasepool(|_| {
        let path = NSString::from_str(&icon_path.to_string_lossy());
        let image = NSWorkspace::sharedWorkspace().iconForFile(&path);
        let tiff = image.TIFFRepresentation().ok_or_else(|| {
            AppError::Platform(format!(
                "macOS could not render an icon for {}",
                icon_path.display()
            ))
        })?;
        let bitmap =
            NSBitmapImageRep::initWithData(NSBitmapImageRep::alloc(), &tiff).ok_or_else(|| {
                AppError::Platform("macOS could not decode the application icon".into())
            })?;
        let properties = NSDictionary::new();
        let png = unsafe {
            bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)
        }
        .ok_or_else(|| AppError::Platform("macOS could not encode the application icon".into()))?;

        Ok(unsafe { png.as_bytes_unchecked() }.to_vec())
    })
}

pub fn application_icon_revision(executable_path: &str) -> String {
    let executable = Path::new(executable_path);
    let bundle = application_bundle_path(executable);
    let mut hasher = DefaultHasher::new();
    executable_path.hash(&mut hasher);
    hash_path_metadata(executable, &mut hasher);
    if let Some(bundle) = bundle {
        bundle.hash(&mut hasher);
        hash_path_metadata(&bundle, &mut hasher);
        hash_path_metadata(&bundle.join("Contents/Info.plist"), &mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

fn hash_path_metadata(path: &Path, hasher: &mut DefaultHasher) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    metadata.len().hash(hasher);
    if let Ok(modified) = metadata.modified()
        && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
    {
        duration.as_secs().hash(hasher);
        duration.subsec_nanos().hash(hasher);
    }
}

fn application_bundle_path(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|ancestor| {
            ancestor
                .extension()
                .is_some_and(|extension| extension == "app")
        })
        .map(Path::to_path_buf)
}

impl IdleTimeProvider for MacOsActivityProvider {
    fn idle_duration(&self) -> AppResult<Duration> {
        const ANY_INPUT_EVENT: CGEventType = CGEventType(u32::MAX);

        let seconds = CGEventSource::seconds_since_last_event_type(
            CGEventSourceStateID::HIDSystemState,
            ANY_INPUT_EVENT,
        );

        Duration::try_from_secs_f64(seconds).map_err(|error| {
            AppError::Platform(format!("macOS returned invalid idle duration: {error}"))
        })
    }
}

impl ForegroundApplicationProvider for MacOsActivityProvider {
    fn foreground_application(&self) -> AppResult<ForegroundApplication> {
        autoreleasepool(|_| {
            let application = NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .ok_or_else(|| {
                    AppError::Platform("macOS did not report a frontmost application".to_owned())
                })?;

            let name = application.localizedName().map(|value| value.to_string());
            let bundle_identifier = application
                .bundleIdentifier()
                .map(|value| value.to_string());
            let executable_path = application
                .executableURL()
                .and_then(|url| url.path())
                .map(|value| value.to_string());

            normalize_application(name, bundle_identifier, executable_path)
        })
    }
}

fn normalize_application(
    name: Option<String>,
    bundle_identifier: Option<String>,
    executable_path: Option<String>,
) -> AppResult<ForegroundApplication> {
    let name = non_empty(name)
        .or_else(|| non_empty(bundle_identifier.clone()))
        .or_else(|| {
            executable_path
                .as_deref()
                .and_then(|path| Path::new(path).file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .filter(|name| !name.is_empty())
        })
        .ok_or_else(|| {
            AppError::Platform("frontmost application has no usable identity".to_owned())
        })?;

    Ok(ForegroundApplication {
        name,
        bundle_identifier: non_empty(bundle_identifier),
        executable_path: non_empty(executable_path),
    })
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_all_reported_application_fields() {
        assert_eq!(
            normalize_application(
                Some("Safari".to_owned()),
                Some("com.apple.Safari".to_owned()),
                Some("/Applications/Safari.app/Contents/MacOS/Safari".to_owned()),
            )
            .expect("application should normalize"),
            ForegroundApplication {
                name: "Safari".to_owned(),
                bundle_identifier: Some("com.apple.Safari".to_owned()),
                executable_path: Some("/Applications/Safari.app/Contents/MacOS/Safari".to_owned()),
            }
        );
    }

    #[test]
    fn falls_back_to_bundle_identifier_when_localized_name_is_missing() {
        let application = normalize_application(
            None,
            Some("com.example.Agent".to_owned()),
            Some("/Applications/Agent".to_owned()),
        )
        .expect("bundle identifier should provide a name");

        assert_eq!(application.name, "com.example.Agent");
    }

    #[test]
    fn falls_back_to_executable_file_name_without_bundle_metadata() {
        let application =
            normalize_application(None, None, Some("/usr/local/bin/example-agent".to_owned()))
                .expect("path should provide a name");

        assert_eq!(application.name, "example-agent");
    }

    #[test]
    fn rejects_application_without_any_identity() {
        assert!(matches!(
            normalize_application(Some(" ".to_owned()), None, None),
            Err(AppError::Platform(_))
        ));
    }

    #[test]
    fn resolves_owning_application_bundle() {
        assert_eq!(
            application_bundle_path(Path::new("/Applications/Safari.app/Contents/MacOS/Safari")),
            Some(PathBuf::from("/Applications/Safari.app"))
        );
        assert_eq!(application_bundle_path(Path::new("/usr/bin/true")), None);
    }
}
