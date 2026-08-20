# Link Helm

[English](README.md) | 简体中文

Link Helm 是一款面向 macOS 和 Windows 的浏览器身份路由工具。它接收系统中的 HTTP/HTTPS 链接，根据来源应用、域名和自定义规则，将链接发送到指定浏览器及对应的 Profile，避免工作账号、个人账号和不同使用场景混在同一个浏览器身份中。

例如，你可以让 Link Helm：

- 将 Mail 中的公司链接发送到 Chrome 的工作 Profile。
- 将聊天软件中的私人链接发送到个人 Profile。
- 对未匹配的链接询问本次使用哪个浏览器身份。
- 暂停自动路由，或只让下一个链接打开身份选择器。

Link Helm 使用 Rust 和 Tauri 2 构建，面向 macOS 13 及以上版本和 Windows 10/11 x64。

## 兼容性状态

| 平台 | 浏览器 | 状态 |
| --- | --- | --- |
| macOS | Chrome、Edge、Brave、Firefox | 已验证 |
| Windows 10/11 x64 | Chrome、Edge、Brave、Firefox | 已验证 |
| Linux | 所有浏览器 | 计划中 |

## 适用读者

- 希望按应用和域名自动选择浏览器 Profile 的 macOS 和 Windows 用户。
- 希望从源码构建 Link Helm、修改路由行为或增加浏览器适配器的 Rust/Tauri 开发者。

## 主要功能

- 按来源应用标识符和域名匹配路由规则。
- 发现 Chrome、Edge、Brave 和 Firefox 的浏览器 Profile。
- 支持指定 Profile、浏览器内活动 Profile、全局活动 Profile 和始终询问等目标模式。
- 提供菜单栏或系统托盘入口、设置窗口和键盘可操作的身份选择器。
- 提供规则预览、Profile 打开测试、配置导入与导出。
- 诊断记录只保留域名和稳定标识符，不持久化 URL 路径、查询参数或片段。
- 支持 English 和简体中文界面，并持久化语言偏好。

## 构建前置条件

从源码构建 Link Helm 需要：

- macOS 13 或更高版本，或 Windows 10/11 x64。
- [Rust stable toolchain](https://www.rust-lang.org/tools/install)。
- macOS 需要 Xcode Command Line Tools；Windows 需要 Visual Studio Build Tools，并安装 Desktop development with C++ 工作负载。
- Tauri CLI 2。

在 macOS 上安装 Xcode Command Line Tools：

```bash
xcode-select --install
```

使用 `rustup` 安装 Rust，将 Cargo 加载到当前 shell，并选择 stable toolchain：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable
```

验证安装结果：

```bash
rustc --version
cargo --version
```

安装 Tauri CLI：

```bash
cargo install tauri-cli --version "^2.0" --locked
```

项目界面使用仓库内的静态 HTML、CSS 和 JavaScript，没有 Node.js 或前端包管理器依赖。

## 获取源码

克隆仓库：

```bash
git clone https://github.com/ryanfan219/link-helm.git
cd link-helm
```

已有本地仓库时，可以获取最新代码：

```bash
git pull --ff-only
```

首次拉取后下载 Rust 依赖：

```bash
cargo fetch
```

## 本地运行

在仓库根目录运行开发版本：

```bash
cargo run -p link-helm-desktop
```

Link Helm 启动后常驻 macOS 菜单栏或 Windows 系统托盘，不会自动显示主窗口。点击 Link Helm 图标，然后选择 **Open Settings...** 或 **打开设置...**。

如果依赖已经下载完成，也可以离线运行：

```bash
cargo run -p link-helm-desktop --offline
```

## 构建 macOS 应用

在 Tauri 项目目录构建本地应用包：

```bash
cd apps/desktop
cargo tauri build --bundles app --no-sign
```

构建产物位于：

```text
target/release/bundle/macos/Link Helm.app
```

## 构建 Windows 应用

在 Windows 10/11 x64 上进入 Tauri 项目目录构建：

```powershell
cd apps/desktop
cargo tauri build --bundles nsis
```

NSIS 安装程序位于 `target\release\bundle\nsis`。正常安装后，从开始菜单启动 Link Helm，并通过系统托盘图标打开设置。

## 首次设置

1. 从 Link Helm 的菜单栏或系统托盘图标打开设置。
2. 在 **Browsers & Profiles / 浏览器与身份** 中重新扫描浏览器 Profile。
3. 在 **Rules / 规则** 中创建规则，选择来源应用、可选域名和目标 Profile。
4. 使用规则预览确认匹配结果，再使用通用页的 Profile 测试功能验证目标浏览器。
5. 需要接收所有外部网页链接时，点击 **Set as Default / 设为默认**。macOS 上完成授权，或在 **系统设置 > 桌面与程序坞 > 默认网页浏览器** 中选择 Link Helm；Windows 上在打开的“默认应用”页面中分别为 HTTP 和 HTTPS 选择 Link Helm。

Link Helm 只会在用户主动执行设为默认操作并完成系统确认后修改默认浏览器。Windows 不允许应用强制完成这一选择。测试完成后，可在同一系统设置中恢复原浏览器。

测试应用能否接收 URL，而不修改系统默认浏览器：

```bash
open -a "Link Helm" 'https://example.com/test'
```

Windows 上可用 URL 启动已安装程序；Link Helm 已运行时，URL 会通过单实例激活路径交给现有进程：

```powershell
.\target\debug\link-helm-desktop.exe "https://example.com/test"
```

## 二次开发

建议从新分支开始开发：

```bash
git checkout -b feature/my-change
```

项目采用 Cargo workspace，主要目录如下：

| 路径 | 职责 |
| --- | --- |
| `crates/router-model` | 配置、浏览器身份和路由数据模型 |
| `crates/router-core` | 规则匹配与路由决策 |
| `crates/browser-adapters` | Chromium 和 Firefox Profile 适配器 |
| `crates/platform-api` | 平台能力抽象接口 |
| `crates/platform-macos` | Launch Services、Accessibility 和 macOS 执行逻辑 |
| `crates/platform-windows` | Win32 进程发现、注册表集成和 Windows 执行逻辑 |
| `crates/config-store` | 配置读取、校验和原子写入 |
| `apps/desktop/dist` | 设置页、身份选择器和国际化资源 |
| `apps/desktop/src-tauri` | Tauri 命令、托盘菜单、窗口和桌面应用状态 |

常见改动入口：

- 修改界面：编辑 `apps/desktop/dist` 下的 HTML、CSS、JavaScript 和 `i18n` 资源。
- 修改规则语义：从 `router-model` 和 `router-core` 开始，保持模型与决策逻辑独立于桌面 UI。
- 增加浏览器支持：在 `browser-adapters` 中实现 Profile 发现和打开策略，并通过 `platform-api` 调用平台能力。
- 修改 macOS 集成：编辑 `platform-macos`，避免把平台细节泄漏到核心路由模块。
- 修改 Windows 集成：编辑 `platform-windows`，避免把注册表和 Win32 细节泄漏到核心路由模块。
- 增加桌面命令：在 `apps/desktop/src-tauri/src/commands.rs` 注册命令，并在 `lib.rs` 的 Tauri handler 中显式暴露。

提交改动前可运行：

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

需要完全离线验证时，在依赖已缓存的前提下为 Cargo 命令添加 `--offline`。

## 故障排查

### 执行 `cargo tauri` 时提示没有该命令

确认已安装 Tauri CLI 2：

```bash
cargo tauri --version
```

如果命令不可用，重新执行前置条件中的 `cargo install tauri-cli`。

### 启动后没有出现窗口

这是预期行为。Link Helm 默认在后台启动并常驻菜单栏或系统托盘，请从 Link Helm 图标打开设置。若图标缺失，请从终端输出中检查启动错误。

### 离线构建提示缺少依赖

`--offline` 只能使用本机已经缓存的 crate。先在联网环境执行 `cargo fetch`，再重新运行离线命令。

### 浏览器 Profile 列表为空

确认浏览器已经安装并至少创建过一个 Profile，然后在 **Browsers & Profiles / 浏览器与身份** 中重新扫描。不同浏览器的 Profile 数据目录和可用能力可能不同。

### 外部链接没有进入 Link Helm

确认 Link Helm 已被设为 HTTP/HTTPS 默认处理程序。开发阶段可使用“首次设置”中的平台命令绕过默认浏览器设置，直接测试 URL 事件。

### 活动 Profile 跟踪不稳定

macOS 上请在设置中检查并授予 Accessibility / 辅助功能权限。Windows 的前台进程观察不需要该权限。

## 配置与隐私

Link Helm 在操作系统的应用数据目录中保存：

- `config.json`：路由规则。
- `preferences.json`：界面语言等应用偏好。
- `diagnostics.json`：有数量上限的诊断记录。

配置和诊断数据使用临时文件替换方式写入。无效配置不会覆盖现有有效配置，应用会进入可见的安全模式。诊断记录不会保存 URL 路径、查询参数值或片段。

## 当前限制

- 活动身份路由依据最近观察到的浏览器 Profile，不会为每个已存在窗口维护永久身份映射。
- 精确的无痕窗口识别尚未实现。

## AI 辅助开发

Link Helm 在设计探索、功能实现、代码评审和文档编写过程中使用 AI 工具辅助。所有 AI 辅助产生的变更均由项目维护者审查，项目维护者对项目的技术决策和版本发布负责。

## 社区

[LINUX DO](https://linux.do) —— 一个面向开发者和技术爱好者的中文社区。

## License

Link Helm 使用 [MIT License](LICENSE) 开源。你可以使用、复制、修改、合并、发布和商业分发本项目，但必须保留许可证和版权声明。
