# Lynko - macOS browser identity router

Lynko routes external HTTP/HTTPS links by source application and domain to a selected browser Profile. The desktop application is built with Tauri 2 and Rust and targets macOS 13 or later.

Design reference: [`2026-08-13-browser-identity-router-design.md`](2026-08-13-browser-identity-router-design.md)

## Workspace

```text
crates/router-model      Configuration, browser, identity, and routing contracts
crates/router-core       Pure rule matching and route decisions
crates/browser-adapters  Chrome, Edge, Brave, and Firefox profile adapters
crates/platform-api      Platform query and execution ports
crates/platform-macos    Launch Services, Accessibility status, AppKit, and launch execution
crates/config-store      Versioned atomic configuration storage
apps/desktop             Tauri settings, tray menu, selector, IPC, and diagnostics
```

## Run

The development executable starts in the background and adds the Lynko menu bar item. Use **Open Settings...** from that menu to show the settings window:

```bash
cargo run -p lynko-desktop --offline
```

Build the macOS application bundle from the Tauri project directory:

```bash
cd apps/desktop
cargo tauri build --bundles app --no-sign
open ../../target/release/bundle/macos/Lynko.app
```

The optional local proxy is only needed when dependencies are not cached:

```bash
export https_proxy=http://127.0.0.1:7897 \
  http_proxy=http://127.0.0.1:7897 \
  all_proxy=socks5://127.0.0.1:7897
```

## Configure And Test

1. Open Browsers & Profiles and select Rescan. Installed browser Profiles should appear by display name and stable directory ID.
2. Open Rules, enter an editable rule name, choose the source application with the macOS application picker, optionally add a domain matcher, choose a target Profile or Ask, then save. The internal rule ID remains stable when the name changes.
3. Use Preview to inspect the matched rule and terminal action without opening a browser.
4. In General, use Open in profile to send `https://example.com` to the selected Profile. Chrome reuses a browser-managed Profile window when available.
5. After building and opening `Lynko.app`, test URL delivery without changing the default browser:

   ```bash
   open -a Lynko 'https://example.com/test'
   ```

6. To receive all external web links, select Open Settings in General, then choose Lynko as the default web browser in macOS System Settings. The button remains available after Lynko becomes default, and Lynko never changes this setting directly.

Closing the settings window hides it and leaves routing and the menu bar item running. Use **Quit Lynko** from the menu bar when you want to stop the router.

To restore Chrome, open System Settings > Desktop & Dock > Default web browser and choose Google Chrome.

## Storage And Privacy

Lynko writes `config.json` and bounded `diagnostics.json` below its macOS application data directory. Configuration and diagnostics use temporary-file replacement. Invalid imported or on-disk configuration is preserved and places the app in visible safe mode.

Diagnostics retain normalized domains and stable IDs only. URL paths, query values, and fragments are not persisted.

## Current Capability Boundary

Chrome Profile discovery and browser-managed Profile opening are implemented. Edge, Brave, and Firefox have adapter contract tests but were not available for live verification on this machine. Source bundle IDs are read from the current AppKit foreground application when available.

For a specified Profile, choose Open target profile to reuse the browser through macOS Launch Services without creating another browser Dock instance. Choose Create target window only when a profile-guaranteed new window is required; that explicit action uses browser command-line arguments.

Lynko records the stable `last_used` Profile while a supported browser is foreground and uses that snapshot for Active in browser and Globally active rules. Chrome active-window opens operate on the existing front window. Specified Profile opens attempt Accessibility-assisted Profile activation and verify the stable Profile ID before opening; if verification is unavailable they fall back to the direct `--profile-directory` path so identity correctness is preserved.

Precise mapping of every existing browser window to a Profile and incognito detection are still not implemented. Active routing therefore follows the most recently observed browser Profile rather than maintaining a permanent identity for every window.

## Verify

```bash
cargo fmt --all -- --check
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo build --workspace --offline
```
