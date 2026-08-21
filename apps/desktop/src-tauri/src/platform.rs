#[cfg(target_os = "macos")]
pub use platform_macos::{MacOsPlatformAdapter as DesktopPlatformAdapter, SystemIntegrationStatus};

#[cfg(target_os = "windows")]
pub use platform_windows::{
    SystemIntegrationStatus, WindowsPlatformAdapter as DesktopPlatformAdapter,
};

#[cfg(target_os = "linux")]
pub use platform_linux::{LinuxPlatformAdapter as DesktopPlatformAdapter, SystemIntegrationStatus};
