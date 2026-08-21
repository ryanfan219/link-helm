# Link Helm

English | [简体中文](README.zh-CN.md) | [Website](https://ryanfan219.github.io/link-helm/)

Link Helm is a browser identity router for macOS, Windows, and Linux. It receives HTTP and HTTPS links from the system and routes them to a specific browser Profile according to the source application, domain, and your rules. This keeps work accounts, personal accounts, and other browsing contexts separated.

For example, Link Helm can:

- Send company links from Mail to a work Profile in Chrome.
- Send private links from chat applications to a personal Profile.
- Ask which browser identity to use when no rule matches.
- Pause automatic routing or show the identity selector for only the next link.

Link Helm is built with Rust and Tauri 2 and targets macOS 13 or later, Windows 10/11 x64, and Linux desktops using XDG conventions.

## Compatibility Status

| Platform | Browser | Status |
| --- | --- | --- |
| macOS | Chrome, Edge, Brave, Firefox | Validated |
| Windows 10/11 x64 | Chrome, Edge, Brave, Firefox | Validated |
| Linux (X11/XDG) | Chrome, Edge, Brave, Firefox | Supported |

## Intended Audience

- macOS, Windows, and Linux users who want to select browser Profiles automatically by application and domain.
- Rust and Tauri developers who want to build Link Helm from source, change its routing behavior, or add browser adapters.

## Features

- Match routing rules by source application ID and domain.
- Discover browser Profiles from Chrome, Edge, Brave, and Firefox.
- Route to a specified Profile, the active Profile in a browser, the globally active Profile, or an interactive prompt.
- Provide a menu bar or system tray controller, settings window, and keyboard-accessible identity selector.
- Preview rules, test Profile opening, and import or export configuration.
- Retain only domains and stable identifiers in diagnostics; URL paths, query values, and fragments are not persisted.
- Provide English and Simplified Chinese interfaces with a persistent language preference.

## Prerequisites

Building Link Helm from source requires:

- macOS 13 or later, Windows 10/11 x64, or a Linux desktop with XDG utilities.
- A [stable Rust toolchain](https://www.rust-lang.org/tools/install).
- Xcode Command Line Tools on macOS, Visual Studio Build Tools with the Desktop development with C++ workload on Windows, or Rust plus WebKitGTK/Tauri prerequisites on Linux.
- Tauri CLI 2.

On Linux, install `xdg-utils` and the WebKitGTK development package supplied by your distribution. Foreground browser detection uses X11 `_NET_ACTIVE_WINDOW` and `/proc`; Wayland compositors may provide reduced source-application detection.

On macOS, install Xcode Command Line Tools:

```bash
xcode-select --install
```

Install Rust with `rustup`, load Cargo into the current shell, and select the stable toolchain:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable
```

Verify the installation:

```bash
rustc --version
cargo --version
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

Link Helm starts in the background and remains in the macOS menu bar or Windows system tray. It does not open the settings window automatically. Click the Link Helm icon, then select **Open Settings...** or **打开设置...**.

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

## Build the Windows App

Run the build on Windows 10/11 x64 from the Tauri project directory:

```powershell
cd apps/desktop
cargo tauri build --bundles nsis
```

The NSIS installer is generated under `target\release\bundle\nsis`. Install it normally, start Link Helm from the Start menu, and use its system tray icon to open settings.

## First-Time Setup

1. Open Link Helm settings from its menu bar or system tray icon.
2. Rescan browser Profiles under **Browsers & Profiles / 浏览器与身份**.
3. Create a rule under **Rules / 规则**, selecting a source application, an optional domain, and a target Profile.
4. Preview the rule, then use the Profile test on the General page to verify the target browser.
5. To receive all external web links, click **Set as Default / 设为默认**. On macOS, complete authorization or select Link Helm under **System Settings > Desktop & Dock > Default web browser**. On Windows, select Link Helm for both HTTP and HTTPS in the Default Apps page that opens.

Link Helm changes the default browser only after you explicitly request it and complete the operating system confirmation. Windows does not allow applications to force this choice. You can restore the previous browser from the same system setting after testing.

Test URL delivery without changing the default browser:

```bash
open -a "Link Helm" 'https://example.com/test'
```

On Windows, start the installed executable with a URL. A running Link Helm instance receives the URL through its single-instance activation path:

```powershell
.\target\debug\link-helm-desktop.exe "https://example.com/test"
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
| `crates/platform-windows` | Win32 process discovery, registry integration, and Windows execution |
| `crates/platform-linux` | XDG handlers, X11 foreground discovery, and Linux execution |
| `crates/config-store` | Configuration loading, validation, and atomic writes |
| `apps/desktop/dist` | Settings UI, identity selector, and localization resources |
| `apps/desktop/src-tauri` | Tauri commands, tray menu, windows, and desktop state |

Common extension points:

- UI changes: edit the HTML, CSS, JavaScript, and `i18n` resources under `apps/desktop/dist`.
- Routing semantics: start in `router-model` and `router-core`, keeping models and decisions independent of the desktop UI.
- Browser support: implement Profile discovery and opening behavior in `browser-adapters`, using platform capabilities through `platform-api`.
- macOS integration: edit `platform-macos` and keep platform details out of the core routing modules.
- Windows integration: edit `platform-windows` and keep registry and Win32 details out of the core routing modules.
- Linux integration: edit `platform-linux`; keep XDG, X11, and `/proc` details out of the core routing modules.
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

This is expected. Link Helm starts in the background and remains in the menu bar or system tray. Open settings from the Link Helm icon. If the icon is missing, check the terminal output for a startup error.

### Offline builds report missing dependencies

The `--offline` option can use only crates already cached on the machine. Run `cargo fetch` while online, then retry the offline command.

### The browser Profile list is empty

Confirm that the browser is installed and has at least one Profile, then rescan under **Browsers & Profiles / 浏览器与身份**. Profile storage and available capabilities differ between browsers.

### External links do not reach Link Helm

Confirm that Link Helm is the default HTTP and HTTPS handler. During development, use the platform-specific command from First-Time Setup to test URL delivery directly without changing the default browser.

### Active Profile tracking is unreliable

On macOS, check and grant Accessibility permission in settings. Windows foreground-process observation does not require this permission. On Linux, X11 foreground detection requires `xprop`; Wayland sessions may need an explicit source application rule.

## Configuration and Privacy

Link Helm stores these files in its operating system application data directory:

- `config.json`: routing rules.
- `preferences.json`: application preferences such as interface language.
- `diagnostics.json`: a bounded set of diagnostic records.

Configuration and diagnostic data are written using temporary-file replacement. Invalid configuration does not overwrite the current valid configuration and places the application in a visible safe mode. Diagnostics do not persist URL paths, query values, or fragments.

## Current Limitations

- Active identity routing follows the most recently observed browser Profile rather than maintaining a permanent identity mapping for every existing window.
- Precise incognito window detection is not implemented.

## AI-Assisted Development

Link Helm is developed with assistance from AI tools for design exploration, implementation, review, and documentation. AI-assisted changes are reviewed by the project maintainer, who remains responsible for the project's technical decisions and releases.

## Community

[LINUX DO](https://linux.do) — A Chinese-language community for developers and technology enthusiasts.

## License

Link Helm is available under the [MIT License](LICENSE). You may use, copy, modify, merge, publish, and commercially distribute the project as long as the license and copyright notice are retained.
