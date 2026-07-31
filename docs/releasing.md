# Releasing YunQi-Watchhouse

YunQi-Watchhouse ships macOS releases through the `Release` GitHub Actions workflow. A release
contains a universal `.app`, a `.dmg`, signed updater artifacts, and `latest.json` for in-app
updates.

## One-time setup

1. Join the Apple Developer Program and create a `Developer ID Application` certificate.
2. Export the certificate and private key from Keychain Access as a password-protected `.p12`.
3. Generate a Tauri updater key pair:

   ```sh
   npm run tauri signer generate -- -w ~/.tauri/yunqi-watchhouse.key
   ```

4. Keep the private updater key outside the repository. Never commit the `.p12`, private updater
   key, Apple app-specific password, or key passwords.
5. Add these GitHub Actions repository secrets:

   | Secret | Value |
   | --- | --- |
   | `APPLE_CERTIFICATE` | Base64-encoded `.p12` contents |
   | `APPLE_CERTIFICATE_PASSWORD` | Password used to export the `.p12` |
   | `APPLE_SIGNING_IDENTITY` | Developer ID identity, such as `Developer ID Application: Name (TEAMID)` |
   | `APPLE_ID` | Apple ID used for notarization |
   | `APPLE_PASSWORD` | App-specific password for that Apple ID |
   | `APPLE_TEAM_ID` | Apple Developer team ID |
   | `TAURI_SIGNING_PRIVATE_KEY` | Entire Tauri updater private key |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Updater key password, or an empty secret if none |
   | `TAURI_UPDATER_PUBLIC_KEY` | Public half of the Tauri updater key pair |

To encode the certificate on macOS:

```sh
base64 -i DeveloperID.p12 | pbcopy
```

The release workflow imports this `.p12` into a temporary macOS keychain and verifies that
`APPLE_SIGNING_IDENTITY` resolves to an imported code-signing identity before the build starts.
The identity value must exactly match the certificate name shown by:

```sh
security find-identity -v -p codesigning
```

Before building, the workflow also signs a temporary sentinel and verifies both Minisign
signatures with `TAURI_UPDATER_PUBLIC_KEY`. This fails the release early when the updater private
key, password, and public key do not belong together.

Release actions are pinned to full commit hashes because this job can access signing credentials
and publish repository releases. Review and update those hashes deliberately when upgrading an
action.

## Create a release

Keep the version identical in `package.json`, `src-tauri/tauri.conf.json`, and
`src-tauri/Cargo.toml`. Validate it before tagging:

```sh
npm run release:check
```

Commit the version change, then create and push the matching tag:

```sh
git tag v0.2.0
git push origin v0.2.0
```

The workflow rejects a tag that does not match the project version. It then imports and verifies the
Developer ID certificate, creates the production updater configuration from the public-key secret,
builds a universal binary, sends it to Apple for notarization, and publishes the GitHub Release.

## Local packages

For a local `.app` and `.dmg` without Apple notarization:

```sh
npm run bundle:mac
```

The results are written to:

- `src-tauri/target/release/bundle/macos/YunQi-Watchhouse.app`
- `src-tauri/target/release/bundle/dmg/YunQi-Watchhouse_<version>_<architecture>.dmg`

The local command applies an ad-hoc signature before creating the DMG so the bundle can be verified
and run on the same Mac. It is not a substitute for a Developer ID signature and Apple notarization.
In-app updates are intentionally disabled unless the production release configuration contains the
real updater public key and endpoint.
