//! DeepCoder 安全沙箱
//!
//! 基于 Codex CLI 的三层安全模型：
//! 1. 进程加固 (process hardening)
//! 2. 平台沙箱 (platform sandbox)
//! 3. 执行策略 (execution policy)

pub mod hardening;
pub mod platform;
pub mod policy;

/// 沙箱模式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SandboxType {
    /// 无沙箱
    None,
    /// macOS Seatbelt
    MacosSeatbelt,
    /// Linux Landlock/bwrap
    LinuxSeccomp,
    /// Windows RestrictedToken
    WindowsRestricted,
}

/// 沙箱管理器
pub struct SandboxManager {
    sandbox_type: SandboxType,
    enabled: bool,
}

impl SandboxManager {
    pub fn new(mode: &str) -> Self {
        let sandbox_type = Self::detect_platform();
        let enabled = match mode {
            "enforce" => true,
            "off" => false,
            _ => sandbox_type != SandboxType::None, // auto
        };

        Self { sandbox_type, enabled }
    }

    fn detect_platform() -> SandboxType {
        #[cfg(target_os = "macos")]
        { SandboxType::MacosSeatbelt }
        #[cfg(target_os = "linux")]
        { SandboxType::LinuxSeccomp }
        #[cfg(target_os = "windows")]
        { SandboxType::WindowsRestricted }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        { SandboxType::None }
    }

    /// 检查命令是否允许执行
    pub fn check_command(&self, command: &str) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        // 安全检查：禁止危险命令
        let dangerous = [
            "rm -rf /", "mkfs.", "dd if=", ":(){ :|:& };:",
            "chmod 777 /", "> /dev/sda",
        ];

        for &pattern in &dangerous {
            if command.contains(pattern) {
                return Err(format!("禁止的危险命令: {pattern}"));
            }
        }

        Ok(())
    }
}
