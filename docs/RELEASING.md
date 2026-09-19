# Releasing FolderSkin

A release is two steps, so nothing reaches users that nobody has tried:

1. **Push a tag.** `.github/workflows/release.yml` builds every platform and attaches the installers
   to a **draft** release. The notes come from the version's section of `CHANGELOG.md`. A draft is
   visible only to people who can push to the repository.
2. **Publish the draft** once you've tried its installers: Actions → Release → Run workflow, enter
   the tag, tick **Publish the release**. That run builds nothing. It checks that every platform's
   installers are attached and makes the draft public, so what goes out is exactly what you tried.

| Platform | Built on | Installers | Signed |
| --- | --- | --- | --- |
| macOS 12+, Apple silicon and Intel | `macos-latest` | universal `.dmg` | Developer ID, notarized |
| Windows 10 and 11, x86_64 | `windows-latest` | `-setup.exe`, `.msi` | only once the Azure secrets exist |
| Linux x86_64 | `ubuntu-22.04` | `.AppImage`, `.deb`, `.rpm` | not needed |
| Linux ARM64 | `ubuntu-22.04-arm` | `.AppImage`, `.deb`, `.rpm` | not needed |

Both Linux builds run on Ubuntu 22.04, so the packages work with glibc 2.35 or newer.

## Before you tag

1. Set the new version in all three places: `package.json`, `src-tauri/tauri.conf.json` and
   `version` under `[workspace.package]` in `Cargo.toml`. Run `cargo check` so `Cargo.lock`
   follows. The workflow refuses a tag that doesn't match all three.
2. In `CHANGELOG.md`, move what's under **Unreleased** into a `## X.Y.Z — YYYY-MM-DD` section.
   The workflow stops if the version has no section.
3. Make sure CI is green on `main`, then:

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

The **Draft is complete** job at the end of the run lists the installers and fails if a platform
is missing. The draft is under Releases, marked Draft.

## Signing

### macOS: the same keys as Oleafly

The release workflow reads the same six repository secrets as Oleafly's, and one Developer ID
certificate signs every app of a team, so FolderSkin reuses Oleafly's keys. GitHub never shows a
secret's value again once it's saved, so they can't be copied across from Oleafly's settings page.
Set them here from the same sources you used for Oleafly:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | the Developer ID Application certificate exported as a `.p12`, base64 encoded |
| `APPLE_CERTIFICATE_PASSWORD` | the password the `.p12` was exported with |
| `APPLE_SIGNING_IDENTITY` | the certificate's name, `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | the Apple account email |
| `APPLE_PASSWORD` | an app-specific password from [account.apple.com](https://account.apple.com) (Sign-In and Security) |
| `APPLE_TEAM_ID` | the 10-character team ID from the membership page |

The certificate goes in straight from the file, so its text never appears on screen:

```sh
base64 -i DeveloperID.p12 | gh secret set APPLE_CERTIFICATE --repo prajwal-svm/folderskin
```

The other five ask for their value when you run them:

```sh
for s in APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID; do gh secret set "$s" --repo prajwal-svm/folderskin; done
```

No `.p12` at hand? In Keychain Access, open **My Certificates**, right-click
**Developer ID Application: …**, choose **Export**, pick **Personal Information Exchange (.p12)**
and set a password. An app-specific password can be shared between the two apps, but one per
app lets you revoke either without breaking the other.

The workflow makes its own temporary keychain password (`KEYCHAIN_PASSWORD`). A tag build in this
repository stops at the macOS step, naming the missing secrets, until all six exist. A fork has
none of them and builds unsigned.

`src-tauri/entitlements.plist` turns on the two JIT entitlements the system WebView needs under
the hardened runtime; Oleafly found that without them a notarized build can open to a blank
window.

### Windows

The workflow has the same optional Azure Trusted Signing step as Oleafly. It signs only when the
`AZURE_*` secrets exist; until then the Windows installers are unsigned and SmartScreen shows
"More info → Run anyway". Oleafly's
[signing guide](https://github.com/Oleafly/Oleafly/blob/main/docs/signing.md#windows-azure-trusted-signing)
lists the six secrets and the Azure setup.

### Linux

Nothing to sign: distributions don't gate AppImages or packages on a signature.

## Checking a signed build

On a Mac, after installing from the draft's `.dmg`:

```sh
spctl -a -vvv --type install /Applications/FolderSkin.app
```

It should say `source=Notarized Developer ID`. `codesign -dv --verbose=4 /Applications/FolderSkin.app`
shows the identity, and `codesign -d --entitlements - /Applications/FolderSkin.app` the two keys
above.

## Not set up yet

FolderSkin has no in-app updater, so there is no update feed to sign. If it gets one, Oleafly's
`TAURI_SIGNING_PRIVATE_KEY` could sign it too, or it can have its own key pair
(`pnpm tauri signer generate`).
