@echo off
SETLOCAL ENABLEDELAYEDEXPANSION

:: Remove VS from path (no Windows SDK), add GNU tools
SET PATH=C:\Users\84342\.cargo\bin;%PATH%
SET PATH=%PATH:D:\VSCoding 2022\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin;=%

rustup default stable-x86_64-pc-windows-gnu >NUL

cargo clean --manifest-path C:\Users\84342\dc-build\Cargo.toml >NUL

echo [1/3] Building deepcoder-types + error + config
cargo build --manifest-path C:\Users\84342\dc-build\Cargo.toml -p deepcoder-types -p deepcoder-error -p deepcoder-config
IF %ERRORLEVEL% NEQ 0 (
    echo FAILED at core libs
    EXIT /B %ERRORLEVEL%
)

echo [2/3] Building all remaining crates
cargo build --manifest-path C:\Users\84342\dc-build\Cargo.toml -p deepcoder-cli
IF %ERRORLEVEL% NEQ 0 (
    echo FAILED at CLI build
    EXIT /B %ERRORLEVEL%
)

echo [3/3] Release build
cargo build --manifest-path C:\Users\84342\dc-build\Cargo.toml --release -p deepcoder-cli
IF %ERRORLEVEL% NEQ 0 (
    echo FAILED at release build
    EXIT /B %ERRORLEVEL%
)

echo BUILD SUCCESS
echo Binary: C:\Users\84342\dc-build\target\release\deepcoder.exe
