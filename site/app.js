const translations = {
  en: {
    skip: "Skip to content", navLabel: "Primary navigation", navWorkflow: "How it works", navCompatibility: "Compatibility", navDownload: "Build Link Helm",
    heroEyebrow: "Browser identity router", heroLede: "One link. The right browser profile.", heroDetail: "Route external links by source application and domain, keeping work, personal, and client sessions in the browser identity where they belong.",
    buildSource: "Build from source", supportLabel: "Current support status", platformRequirement: "macOS 13+ / Windows 10/11 x64 / Linux XDG", platformValidated: "macOS, Windows, and Linux validated", openSource: "MIT licensed",
    productEyebrow: "Control without context switching", productTitle: "A quiet control surface for every incoming link", productDetail: "Set Link Helm as the default browser, define precise rules, preview routing decisions, and keep sensitive URL paths out of diagnostics.", productAlt: "Link Helm settings showing default browser, accessibility, routing controls, and browser profile testing", productCaption: "The current Link Helm settings interface on macOS.",
    workflowEyebrow: "Routing pipeline", workflowTitle: "Context in. Correct identity out.", workflowDetail: "Link Helm evaluates only the context needed to make a routing decision.",
    stepSourceTitle: "Source application", stepSourceDetail: "Mail, chat, calendar, or any app opening a web link.", stepContextTitle: "Link context", stepContextDetail: "Match by source application identifier and domain without retaining full URLs.", stepRuleTitle: "Routing rule", stepRuleDetail: "Choose a specified, active, global, or prompted destination.", stepProfileTitle: "Browser profile", stepProfileDetail: "Open the link in the browser-managed identity that fits the context.",
    capabilitiesEyebrow: "Built for repeat use", capabilitiesTitle: "Routing that stays explainable", featureRulesTitle: "Specific, ordered rules", featureRulesDetail: "Preview matches before relying on them, then reorder and export your configuration.", featurePrivacyTitle: "Bounded diagnostics", featurePrivacyDetail: "Diagnostics retain domains and stable identifiers, not paths, query values, or fragments.", featureFallbackTitle: "Visible fallback behavior", featureFallbackDetail: "Ask, reuse an active profile, use an installed browser's default behavior, or fail explicitly when a destination is unavailable.",
    compatibilityEyebrow: "Current support", compatibilityTitle: "Validated on macOS, Windows, and Linux", compatibilityDetail: "Chrome, Edge, Brave, and Firefox routing has been validated across all three desktop platforms.", tablePlatform: "Platform", tableBrowser: "Browser", tableStatus: "Status", statusValidated: "Validated",
    downloadEyebrow: "Download or build", downloadTitle: "Get Link Helm for your platform", downloadDetail: "Download the unsigned 1.0.0 packages below, or build from source with Rust stable, Tauri CLI 2, and each platform's native prerequisites.", macPackageDetail: "Download the package for your Mac, or build an unsigned DMG on macOS with Xcode Command Line Tools installed.", windowsPackageDetail: "Download the x64 installer, or build NSIS on Windows 10/11 with Visual Studio Build Tools installed.", linuxPackageDetail: "Download a Debian package or AppImage, or build on Linux with WebKitGTK, AppIndicator, and XDG dependencies.", outputLabel: "Build output", downloadAppleSilicon: "Apple Silicon DMG", downloadIntel: "Intel DMG", downloadWindows: "Windows x64", downloadDeb: "Debian / Ubuntu", downloadAppImage: "AppImage", fullInstructions: "Build instructions",
    footerText: "Open source under the MIT License. Built with Rust and Tauri.", viewGithub: "View on GitHub"
  },
  zh: {
    skip: "跳到主要内容", navLabel: "主导航", navWorkflow: "工作方式", navCompatibility: "兼容性", navDownload: "构建 Link Helm",
    heroEyebrow: "浏览器身份路由器", heroLede: "一个链接，进入正确的浏览器身份。", heroDetail: "根据来源应用和域名路由外部链接，让工作、个人和客户会话始终进入对应的浏览器身份。",
    buildSource: "从源码构建", supportLabel: "当前支持状态", platformRequirement: "macOS 13+ / Windows 10/11 x64 / Linux XDG", platformValidated: "macOS、Windows 与 Linux 均已验证", openSource: "MIT 许可证",
    productEyebrow: "无需切换上下文的控制", productTitle: "安静管理每一个传入链接", productDetail: "将 Link Helm 设为默认浏览器，定义精确规则、预览路由结果，并避免在诊断记录中保存敏感 URL 路径。", productAlt: "Link Helm 设置界面，展示默认浏览器、辅助功能、路由控制和浏览器身份测试", productCaption: "当前 macOS 版 Link Helm 设置界面。",
    workflowEyebrow: "路由流程", workflowTitle: "输入上下文，输出正确身份。", workflowDetail: "Link Helm 只评估作出路由决策所需的上下文。",
    stepSourceTitle: "来源应用", stepSourceDetail: "邮件、聊天、日历或任何打开网页链接的应用。", stepContextTitle: "链接上下文", stepContextDetail: "按来源应用标识符和域名匹配，不保留完整 URL。", stepRuleTitle: "路由规则", stepRuleDetail: "选择指定、活跃、全局活跃或每次询问的目标。", stepProfileTitle: "浏览器身份", stepProfileDetail: "在符合当前上下文的浏览器托管身份中打开链接。",
    capabilitiesEyebrow: "适合重复使用", capabilitiesTitle: "始终可解释的路由", featureRulesTitle: "明确、有序的规则", featureRulesDetail: "依赖规则前先预览匹配结果，并可调整顺序和导出配置。", featurePrivacyTitle: "有边界的诊断", featurePrivacyDetail: "诊断仅保留域名和稳定标识，不记录路径、查询参数或片段。", featureFallbackTitle: "清晰的回退行为", featureFallbackDetail: "目标不可用时，可以询问、复用活跃身份、使用已安装浏览器的默认行为或明确失败。",
    compatibilityEyebrow: "当前支持", compatibilityTitle: "macOS、Windows 与 Linux 均已验证", compatibilityDetail: "Chrome、Edge、Brave 和 Firefox 的路由能力已在三个桌面平台完成验证。", tablePlatform: "平台", tableBrowser: "浏览器", tableStatus: "状态", statusValidated: "已验证",
    downloadEyebrow: "下载或构建", downloadTitle: "下载适合你平台的 Link Helm", downloadDetail: "可直接下载下方未签名的 1.0.0 安装包，也可以安装 Rust stable、Tauri CLI 2 和平台原生依赖后从源码构建。", macPackageDetail: "下载适合你的 Mac 安装包，或在已安装 Xcode Command Line Tools 的 macOS 上构建未签名 DMG。", windowsPackageDetail: "下载 x64 安装程序，或在已安装 Visual Studio Build Tools 的 Windows 10/11 上构建 NSIS。", linuxPackageDetail: "下载 Debian 安装包或 AppImage，或安装 WebKitGTK、AppIndicator 和 XDG 依赖后在 Linux 上构建。", outputLabel: "构建产物", downloadAppleSilicon: "Apple Silicon DMG", downloadIntel: "Intel DMG", downloadWindows: "Windows x64", downloadDeb: "Debian / Ubuntu", downloadAppImage: "AppImage", fullInstructions: "查看构建说明",
    footerText: "基于 MIT 许可证开源，使用 Rust 和 Tauri 构建。", viewGithub: "在 GitHub 查看"
  }
};

const languageButton = document.getElementById("language-toggle");
const languageLabel = document.getElementById("language-label");

function applyLanguage(language) {
  const copy = translations[language] || translations.en;
  document.documentElement.lang = language === "zh" ? "zh-CN" : "en";
  document.querySelectorAll("[data-i18n]").forEach((element) => {
    const value = copy[element.dataset.i18n];
    if (value) element.textContent = value;
  });
  document.querySelectorAll("[data-i18n-aria]").forEach((element) => {
    const value = copy[element.dataset.i18nAria];
    if (value) element.setAttribute("aria-label", value);
  });
  document.querySelectorAll("[data-i18n-alt]").forEach((element) => {
    const value = copy[element.dataset.i18nAlt];
    if (value) element.setAttribute("alt", value);
  });
  const nextLanguage = language === "zh" ? "en" : "zh";
  languageLabel.textContent = language === "zh" ? "EN" : "中文";
  languageButton.setAttribute("aria-label", language === "zh" ? "Switch to English" : "切换到中文");
  languageButton.setAttribute("title", language === "zh" ? "Switch to English" : "切换到中文");
  languageButton.dataset.nextLanguage = nextLanguage;
  window.localStorage.setItem("link-helm-site-language", language);
}

languageButton.addEventListener("click", () => applyLanguage(languageButton.dataset.nextLanguage));

const savedLanguage = window.localStorage.getItem("link-helm-site-language");
const initialLanguage = savedLanguage || (navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en");
applyLanguage(initialLanguage);
