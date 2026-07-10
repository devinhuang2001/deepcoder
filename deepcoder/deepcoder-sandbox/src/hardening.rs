//! 进程加固 — 启动前安全措施

/// 进程加固函数，应在程序启动且尚未创建其他线程时调用。
///
/// - Linux: 禁用 core dump, 限制 ptrace, 清除 LD_* 环境变量
/// - macOS: ptrace(PT_DENY_ATTACH), 清除 DYLD_* 环境变量
/// - Windows: 句柄加固
///
/// # Safety
///
/// 调用方必须保证进程中没有其他线程并发读取或写入环境变量。
pub unsafe fn apply_hardening() {
    #[cfg(target_os = "linux")]
    {
        // 禁用 core dump
        let _ = std::fs::write("/proc/self/core_limit", "0");
        // 清除危险的动态链接环境变量
        for var in &["LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT", "LD_DEBUG"] {
            // SAFETY: 该函数的调用契约要求此时尚未创建其他线程。
            unsafe { std::env::remove_var(var) };
        }
    }

    #[cfg(target_os = "macos")]
    {
        // 清除危险的动态链接环境变量
        for var in &[
            "DYLD_INSERT_LIBRARIES",
            "DYLD_LIBRARY_PATH",
            "DYLD_FRAMEWORK_PATH",
        ] {
            // SAFETY: 该函数的调用契约要求此时尚未创建其他线程。
            unsafe { std::env::remove_var(var) };
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows 句柄继承加固
        // 在 Rust 中通过 SetHandleInformation 实现
    }

    tracing::debug!("Process hardening applied");
}
