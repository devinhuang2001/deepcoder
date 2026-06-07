@echo off
echo =============================================
echo DeepCoder Build Script
echo Just double-click or run this file.
echo =============================================

SET PATH=C:\Users\84342\.cargo\bin;%PATH%

:: Clean and switch to GNU toolchain
rustup default stable-x86_64-pc-windows-gnu
cargo clean --manifest-path C:\Users\84342\dc-build\Cargo.toml

echo [Step 1/3] Building core libraries...
cargo build --manifest-path C:\Users\84342\dc-build\Cargo.toml -p deepcoder-types -p deepcoder-error -p deepcoder-config
if %ERRORLEVEL% neq 0 goto :error

echo [Step 2/3] Building CLI...
cargo build --manifest-path C:\Users\84342\dc-build\Cargo.toml -p deepcoder-cli
if %ERRORLEVEL% neq 0 goto :error

echo [Step 3/3] Building release binary...
cargo build --manifest-path C:\Users\84342\dc-build\Cargo.toml --release -p deepcoder-cli
if %ERRORLEVEL% neq 0 goto :error

echo =============================================
echo BUILD SUCCESS!
echo Binary: C:\Users\84342\dc-build\target\release\deepcoder.exe
echo =============================================
pause
exit /b 0

:error
echo FAILED! Check errors above.
pause
exit /b 1
