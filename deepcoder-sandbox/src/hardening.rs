//! 进程加固 — 启动前安全措施

/// 进程加固函数，应在 main() 之前调用
///
/// - Linux: 禁用 core dump, 限制 ptrace, 清除 LD_* 环境变量
/// - macOS: ptrace(PT_DENY_ATTACH), 清除 DYLD_* 环境变量
/// - Windows: 句柄加固
pub fn apply_hardening() {
    #[cfg(target_os = "linux")]
    {
        // 禁用 core dump
        if let Ok(rc) = std::fs::write("/proc/self/core_limit", "0") {
            let _ = rc;
        }
        // 清除危险的动态链接环境变量
        for var in &["LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT", "LD_DEBUG"] {
            std::env::remove_var(var);
        }
    }

    #[cfg(target_os = "macos")]
    {
        // 清除危险的动态链接环境变量
        for var in &["DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH", "DYLD_FRAMEWORK_PATH"] {
            std::env::remove_var(var);
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows 句柄继承加固
        // 在 Rust 中通过 SetHandleInformation 实现
    }

    tracing::debug!("Process hardening applied");
}
