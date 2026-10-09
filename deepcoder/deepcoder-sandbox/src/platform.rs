//! Platform sandbox adapters.

use crate::SandboxType;

pub fn detect_platform() -> SandboxType {
    #[cfg(target_os = "macos")]
    {
        SandboxType::MacosSeatbelt
    }
    #[cfg(target_os = "linux")]
    {
        SandboxType::LinuxSeccomp
    }
    #[cfg(target_os = "windows")]
    {
        SandboxType::WindowsRestricted
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        SandboxType::None
    }
}

pub fn backend_available(sandbox_type: SandboxType) -> bool {
    let _ = sandbox_type;
    false
}

pub fn backend_name(sandbox_type: SandboxType) -> &'static str {
    match sandbox_type {
        SandboxType::None => "none",
        SandboxType::MacosSeatbelt => "macos-seatbelt",
        SandboxType::LinuxSeccomp => "linux-seccomp",
        SandboxType::WindowsRestricted => "windows-restricted",
    }
}
