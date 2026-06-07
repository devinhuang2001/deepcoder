@echo on
SETLOCAL

:: 最小 PATH：Rust + Windows 系统，不含 Git/MSVC
SET PATH=C:\Users\84342\.cargo\bin
SET PATH=%PATH%;C:\Users\84342\.rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained
SET PATH=%PATH%;%SystemRoot%\system32;%SystemRoot%;%SystemRoot%\System32\Wbem

echo === Step 1: Clean cargo cache ===
C:\Users\84342\.cargo\bin\cargo.exe clean --manifest-path C:\Users\84342\dc-build\Cargo.toml

echo === Step 2: Build release ===
C:\Users\84342\.cargo\bin\cargo.exe build --manifest-path C:\Users\84342\dc-build\Cargo.toml --release -p deepcoder-cli

IF %ERRORLEVEL% EQU 0 (
    echo ===== BUILD SUCCESS =====
    echo Binary: C:\Users\84342\dc-build\target\release\deepcoder.exe
) ELSE (
    echo ===== BUILD FAILED =====
    EXIT /B 1
)
