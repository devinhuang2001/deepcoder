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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    enforce: bool,
    configured_off: bool,
}

impl SandboxManager {
    pub fn new(mode: &str) -> Self {
        let sandbox_type = platform::detect_platform();
        let enabled = match mode {
            "enforce" => true,
            "off" => false,
            _ => platform::backend_available(sandbox_type), // auto
        };

        Self {
            sandbox_type,
            enabled,
            enforce: mode == "enforce",
            configured_off: mode == "off",
        }
    }

    pub fn sandbox_type(&self) -> SandboxType {
        self.sandbox_type
    }

    pub fn backend_name(&self) -> &'static str {
        platform::backend_name(self.sandbox_type)
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// 检查命令是否允许执行
    pub fn check_command(&self, command: &str) -> Result<(), String> {
        tracing::trace!("checking command with sandbox {:?}", self.sandbox_type);
        if self.configured_off {
            return Ok(());
        }

        // 安全检查：禁止危险命令
        let dangerous = [
            "rm -rf /",
            "mkfs.",
            "dd if=",
            ":(){ :|:& };:",
            "chmod 777 /",
            "> /dev/sda",
        ];

        for &pattern in &dangerous {
            if command.contains(pattern) {
                return Err(format!("禁止的危险命令: {pattern}"));
            }
        }

        if self.enforce && !platform::backend_available(self.sandbox_type) {
            return Err("enforce mode requires a supported platform sandbox backend".into());
        }
        if !self.enabled {
            return Ok(());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_off_allows_command() {
        let sandbox = SandboxManager::new("off");
        assert!(sandbox.check_command("rm -rf /").is_ok());
    }

    #[test]
    fn sandbox_enforce_rejects_dangerous_command() {
        let sandbox = SandboxManager::new("enforce");
        let error = sandbox.check_command("sudo rm -rf /").unwrap_err();
        assert!(error.contains("rm -rf /"));
    }

    #[test]
    fn sandbox_auto_reports_platform_backend() {
        let sandbox = SandboxManager::new("auto");
        assert_eq!(sandbox.sandbox_type(), platform::detect_platform());
        assert_eq!(
            sandbox.enabled(),
            platform::backend_available(platform::detect_platform())
        );
        assert!(!sandbox.backend_name().is_empty());
    }

    #[test]
    fn sandbox_auto_still_blocks_dangerous_command_without_backend() {
        let sandbox = SandboxManager::new("auto");
        let error = sandbox.check_command("echo rm -rf /").unwrap_err();
        assert!(error.contains("rm -rf /"));
    }
}
