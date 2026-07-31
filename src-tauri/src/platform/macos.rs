use std::{
    ffi::{c_char, c_void},
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    time::Duration,
};

use objc2::{AnyThread, rc::autoreleasepool};
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
use objc2_core_graphics::{CGEventSource, CGEventSourceStateID, CGEventType};
use objc2_foundation::{NSDictionary, NSString};
#[allow(deprecated)]
use objc2_foundation::{NSUserNotification, NSUserNotificationCenter};

use crate::{
    error::{AppError, AppResult},
    platform::{ForegroundApplication, ForegroundApplicationProvider, IdleTimeProvider},
};

type AxUiElementRef = *const c_void;
type CfTypeRef = *const c_void;
type CfStringRef = *const c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXUIElementCreateApplication(pid: i32) -> AxUiElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AxUiElementRef,
        attribute: CfStringRef,
        value: *mut CfTypeRef,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: CfTypeRef);
    fn CFStringGetLength(value: CfStringRef) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    fn CFStringGetCString(
        value: CfStringRef,
        buffer: *mut c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> bool;
}

const UTF8_ENCODING: u32 = 0x0800_0100;

/// Reads macOS' aggregate HID idle duration through the public CoreGraphics API.
///
/// `kCGAnyInputEventType` is intentionally represented as `u32::MAX`: Apple
/// defines it as an enum sentinel, while objc2 skips generating that constant.
/// This call queries elapsed time only and does not install an event tap.
#[derive(Debug, Default, Clone, Copy)]
pub struct MacOsActivityProvider;

#[allow(deprecated)]
pub fn show_break_notification() -> AppResult<()> {
    autoreleasepool(|_| {
        let notification = NSUserNotification::new();
        let title = NSString::from_str("Time for a short break");
        let body = NSString::from_str("Step away for a moment before your next focus block.");
        notification.setTitle(Some(&title));
        notification.setInformativeText(Some(&body));
        NSUserNotificationCenter::defaultUserNotificationCenter()
            .deliverNotification(&notification);
    });
    Ok(())
}

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
            let process_identifier = application.processIdentifier();

            normalize_application(
                name,
                bundle_identifier,
                executable_path,
                Some(process_identifier),
            )
        })
    }

    fn window_title(&self, application: &ForegroundApplication) -> AppResult<Option<String>> {
        if accessibility_permission() != crate::platform::AccessibilityPermission::Granted {
            return Ok(None);
        }
        let Some(pid) = application.process_identifier else {
            return Ok(None);
        };
        Ok(read_window_title(pid))
    }
}

pub fn accessibility_permission() -> crate::platform::AccessibilityPermission {
    if unsafe { AXIsProcessTrusted() } {
        crate::platform::AccessibilityPermission::Granted
    } else {
        crate::platform::AccessibilityPermission::Denied
    }
}

fn read_window_title(pid: i32) -> Option<String> {
    autoreleasepool(|_| {
        let application = unsafe { AXUIElementCreateApplication(pid) };
        if application.is_null() {
            return None;
        }
        let focused_window = copy_ax_attribute(application, "AXFocusedWindow");
        unsafe { CFRelease(application) };
        let window = focused_window?;
        let title = copy_ax_attribute(window, "AXTitle");
        unsafe { CFRelease(window) };
        let title = title?;
        let result = cf_string_to_string(title);
        unsafe { CFRelease(title) };
        result.filter(|value| !value.trim().is_empty())
    })
}

fn copy_ax_attribute(element: AxUiElementRef, name: &str) -> Option<CfTypeRef> {
    let name = NSString::from_str(name);
    let mut value: CfTypeRef = std::ptr::null();
    let status = unsafe {
        AXUIElementCopyAttributeValue(element, (&*name as *const NSString).cast(), &mut value)
    };
    (status == 0 && !value.is_null()).then_some(value)
}

fn cf_string_to_string(value: CfTypeRef) -> Option<String> {
    let length = unsafe { CFStringGetLength(value) };
    if length < 0 {
        return None;
    }
    let capacity =
        unsafe { CFStringGetMaximumSizeForEncoding(length, UTF8_ENCODING) }.checked_add(1)?;
    let mut buffer = vec![0_u8; usize::try_from(capacity).ok()?];
    if !unsafe { CFStringGetCString(value, buffer.as_mut_ptr().cast(), capacity, UTF8_ENCODING) } {
        return None;
    }
    let end = buffer.iter().position(|byte| *byte == 0)?;
    String::from_utf8(buffer[..end].to_vec()).ok()
}

fn normalize_application(
    name: Option<String>,
    bundle_identifier: Option<String>,
    executable_path: Option<String>,
    process_identifier: Option<i32>,
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
        process_identifier,
        window_title: None,
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
                Some(42),
            )
            .expect("application should normalize"),
            ForegroundApplication {
                name: "Safari".to_owned(),
                bundle_identifier: Some("com.apple.Safari".to_owned()),
                executable_path: Some("/Applications/Safari.app/Contents/MacOS/Safari".to_owned()),
                process_identifier: Some(42),
                window_title: None,
            }
        );
    }

    #[test]
    fn falls_back_to_bundle_identifier_when_localized_name_is_missing() {
        let application = normalize_application(
            None,
            Some("com.example.Agent".to_owned()),
            Some("/Applications/Agent".to_owned()),
            None,
        )
        .expect("bundle identifier should provide a name");

        assert_eq!(application.name, "com.example.Agent");
    }

    #[test]
    fn falls_back_to_executable_file_name_without_bundle_metadata() {
        let application = normalize_application(
            None,
            None,
            Some("/usr/local/bin/example-agent".to_owned()),
            None,
        )
        .expect("path should provide a name");

        assert_eq!(application.name, "example-agent");
    }

    #[test]
    fn rejects_application_without_any_identity() {
        assert!(matches!(
            normalize_application(Some(" ".to_owned()), None, None, None),
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
