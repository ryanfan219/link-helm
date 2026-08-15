# Link Helm

English | [简体中文](README.zh-CN.md) | [Website](https://ryanfan219.github.io/link-helm/)

Link Helm is a browser identity router for macOS. It receives HTTP and HTTPS links from the system and routes them to a specific browser Profile according to the source application, domain, and your rules. This keeps work accounts, personal accounts, and other browsing contexts separated.

For example, Link Helm can:

- Send company links from Mail to a work Profile in Chrome.
- Send private links from chat applications to a personal Profile.
- Ask which browser identity to use when no rule matches.
- Pause automatic routing or show the identity selector for only the next link.

Link Helm is built with Rust and Tauri 2 and currently targets macOS 13 or later.

## Compatibility Status

| Platform | Browser | Status |
| --- | --- | --- |
| macOS | Google Chrome | Validated |
| macOS | Microsoft Edge, Brave, Firefox | Adapter-level support; not fully validated |
| Windows and Linux | All browsers | Planned |

## Intended Audience

- macOS users who want to select browser Profiles automatically by application and domain.
- Rust and Tauri developers who want to build Link Helm from source, change its routing behavior, or add browser adapters.

## Features

- Match routing rules by source application Bundle ID and domain.
- Discover browser Profiles from Chrome, Edge, Brave, and Firefox.
- Route to a specified Profile, the active Profile in a browser, the globally active Profile, or an interactive prompt.
- Provide a menu bar controller, settings window, and keyboard-accessible identity selector.
- Preview rules, test Profile opening, and import or export configuration.
- Retain only domains and stable identifiers in diagnostics; URL paths, query values, and fragments are not persisted.
- Provide English and Simplified Chinese interfaces with a persistent language preference.

## Prerequisites

Building Link Helm from source requires:

- macOS 13 or later.
- A [stable Rust toolchain](https://www.rust-lang.org/tools/install).
- Xcode Command Line Tools.
- Tauri CLI 2.

Install Xcode Command Line Tools:

```bash
xcode-select --install
```

Install Tauri CLI:

```bash
cargo install tauri-cli --version "^2.0" --locked
```

The interface uses static HTML, CSS, and JavaScript stored in the repository. Node.js and a frontend package manager are not required.

## Get the Source

Clone the repository:

```bash
git clone https://github.com/ryanfan219/link-helm.git
cd link-helm
```

To update an existing local clone:

```bash
git pull --ff-only
```

Download Rust dependencies after the first clone:

```bash
cargo fetch
```

## Run Locally

Run the development build from the repository root:

```bash
cargo run -p link-helm-desktop
```

Link Helm starts in the background and remains in the macOS menu bar. It does not open the settings window automatically. Click the Link Helm menu bar icon, then select **Open Settings...** or **打开设置...**.

After all dependencies have been downloaded, you can also run offline:

```bash
cargo run -p link-helm-desktop --offline
```

## Build the macOS App

Build a local application bundle from the Tauri project directory:

```bash
cd apps/desktop
cargo tauri build --bundles app --no-sign
```

The application bundle is generated at:

```text
target/release/bundle/macos/Link Helm.app
```

To package separate DMGs for Intel and Apple Silicon Macs, install both Rust targets once:

```bash
rustup target add x86_64-apple-darwin aarch64-apple-darwin
```

Then run the architecture-specific builds from the Tauri project directory:

```bash
cd apps/desktop

# Intel Macs
cargo tauri build --target x86_64-apple-darwin --bundles dmg --no-sign

# Apple Silicon Macs
cargo tauri build --target aarch64-apple-darwin --bundles dmg --no-sign
```

The DMG packages are generated under the repository root at:

```text
target/x86_64-apple-darwin/release/bundle/dmg/Link Helm_<version>_x64.dmg
target/aarch64-apple-darwin/release/bundle/dmg/Link Helm_<version>_aarch64.dmg
```

## First-Time Setup

1. Open Link Helm settings from the menu bar.
2. Rescan browser Profiles under **Browsers & Profiles / 浏览器与身份**.
3. Create a rule under **Rules / 规则**, selecting a source application, an optional domain, and a target Profile.
4. Preview the rule, then use the Profile test on the General page to verify the target browser.
5. To receive all external web links, click **Set as Default / 设为默认** and complete macOS authorization. You can also select Link Helm manually under **System Settings > Desktop & Dock > Default web browser**.

Link Helm changes the default browser only after you explicitly request it and complete macOS authorization. You can restore the previous browser from the same system setting after testing.

Test URL delivery without changing the default browser:

```bash
open -a "Link Helm" 'https://example.com/test'
```

## Development

Start development on a separate branch:

```bash
git checkout -b feature/my-change
```

The project is organized as a Cargo workspace:

| Path | Responsibility |
| --- | --- |
| `crates/router-model` | Configuration, browser identity, and routing data models |
| `crates/router-core` | Rule matching and routing decisions |
| `crates/browser-adapters` | Chromium and Firefox Profile adapters |
| `crates/platform-api` | Platform capability interfaces |
| `crates/platform-macos` | Launch Services, Accessibility, and macOS execution |
| `crates/config-store` | Configuration loading, validation, and atomic writes |
| `apps/desktop/dist` | Settings UI, identity selector, and localization resources |
| `apps/desktop/src-tauri` | Tauri commands, tray menu, windows, and desktop state |

Common extension points:

- UI changes: edit the HTML, CSS, JavaScript, and `i18n` resources under `apps/desktop/dist`.
- Routing semantics: start in `router-model` and `router-core`, keeping models and decisions independent of the desktop UI.
- Browser support: implement Profile discovery and opening behavior in `browser-adapters`, using platform capabilities through `platform-api`.
- macOS integration: edit `platform-macos` and keep platform details out of the core routing modules.
- Desktop commands: register commands in `apps/desktop/src-tauri/src/commands.rs` and expose them explicitly through the Tauri handler in `lib.rs`.

Before submitting changes, run:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

For fully offline verification, add `--offline` to Cargo commands after all dependencies have been cached.

## Troubleshooting

### `cargo tauri` is not available

Confirm that Tauri CLI 2 is installed:

```bash
cargo tauri --version
```

If the command is unavailable, run the `cargo install tauri-cli` command from the prerequisites section again.

### No window appears after startup

This is expected. Link Helm starts in the background and remains in the menu bar. Open settings from the menu bar icon. If the icon is missing, check the terminal output for a startup error.

### Offline builds report missing dependencies

The `--offline` option can use only crates already cached on the machine. Run `cargo fetch` while online, then retry the offline command.

### The browser Profile list is empty

Confirm that the browser is installed and has at least one Profile, then rescan under **Browsers & Profiles / 浏览器与身份**. Profile storage and available capabilities differ between browsers.

### External links do not reach Link Helm

Confirm that Link Helm is the default HTTP and HTTPS handler. During development, use `open -a "Link Helm" 'https://example.com/test'` to test URL events directly without changing the default browser.

### Active Profile tracking is unreliable

Check and grant Accessibility permission in settings. Link Helm uses this permission to observe foreground browsers and windows reliably; some active identity behavior is limited without it.

## Configuration and Privacy

Link Helm stores these files in its macOS application data directory:

- `config.json`: routing rules.
- `preferences.json`: application preferences such as interface language.
- `diagnostics.json`: a bounded set of diagnostic records.

Configuration and diagnostic data are written using temporary-file replacement. Invalid configuration does not overwrite the current valid configuration and places the application in a visible safe mode. Diagnostics do not persist URL paths, query values, or fragments.

## Current Limitations

- Active identity routing follows the most recently observed browser Profile rather than maintaining a permanent identity mapping for every existing window.
- Precise incognito window detection is not implemented.

## License

Link Helm is available under the [MIT License](LICENSE). You may use, copy, modify, merge, publish, and commercially distribute the project as long as the license and copyright notice are retained.
