#[cfg(target_os = "macos")]
pub use platform_macos::{MacOsPlatformAdapter as DesktopPlatformAdapter, SystemIntegrationStatus};

#[cfg(target_os = "windows")]
pub use platform_windows::{
    SystemIntegrationStatus, WindowsPlatformAdapter as DesktopPlatformAdapter,
};
