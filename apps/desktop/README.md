# Link Helm Desktop

This directory contains the Tauri 2 desktop application: a menu bar controller, settings window, keyboard-operated identity selector, URL-open event handler, and privacy-safe diagnostics.

## Start Development Build

From the repository root:

```bash
cargo run -p link-helm-desktop --offline
```

Link Helm starts in the background. Open the settings window from the Link Helm menu bar item. Closing settings hides the window; only **Quit Link Helm** stops routing.

The settings window contains:

- General: default browser and Accessibility status, Pause, Ask Next, direct Profile test, and JSON configuration transfer.
- Browsers & Profiles: installed state, discovered Profiles, and capability status.
- Rules: create, rename, edit, delete, save, and preview routing rules. Source applications are selected with the native macOS application chooser.
- Diagnostics: bounded domain-only routing events and persistence errors.

## Build And Activate The App Bundle

```bash
cd apps/desktop
cargo tauri build --bundles app --no-sign
open "../../target/release/bundle/macos/Link Helm.app"
```

Opening the bundle registers it with Launch Services. Test an incoming URL without changing the system default:

```bash
open -a "Link Helm" 'https://example.com/test'
```

Use Open Settings when ready, then choose Link Helm in System Settings > Desktop & Dock > Default web browser. This action remains available while Link Helm is already default. Link Helm does not change the default directly; restore the previous browser from the same system control.

## Rule Example

- Rule name: `Mail work links`
- Source app: choose Mail; Link Helm records `com.apple.mail`
- Domain: `*.example.com`
- Target mode: Specified profile
- Enforcement: Force
- When unavailable: Create target window

Preview the rule first, then use Open in profile with a non-sensitive URL. Ordinary existing-window opens use Launch Services and do not start a second browser Dock instance. Failed validation keeps the editor contents and current saved configuration unchanged.

Use Open target profile instead of Create target window when the rule should reuse an existing browser-managed Profile window.

## Limitations

Link Helm observes the foreground browser and records Chromium's stable `last_used` Profile for Active in browser and Globally active rules. Specified Chrome Profile opens use Accessibility-assisted Profile activation with stable-ID verification, then fall back to direct Profile invocation when automation cannot be verified. Full AX mapping of every browser window and incognito identification remain disabled. Chrome has live discovery coverage; Edge, Brave, and Firefox currently have adapter-level contract coverage only.
