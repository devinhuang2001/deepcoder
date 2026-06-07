@echo off
SETLOCAL ENABLEDELAYEDEXPANSION

CALL "D:\VSCoding 2022\VC\Auxiliary\Build\vcvarsall.bat" x64
IF %ERRORLEVEL% NEQ 0 (
    echo vcvarsall failed
    EXIT /B 1
)

SET PATH=C:\Users\84342\.cargo\bin;%PATH%

rustup default stable-x86_64-pc-windows-msvc >NUL

cargo clean --manifest-path C:\Users\84342\dc-build\Cargo.toml >NUL

echo [1/3] Building deepcoder-types + error + config
cargo build --manifest-path C:\Users\84342\dc-build\Cargo.toml -p deepcoder-types -p deepcoder-error -p deepcoder-config
IF %ERRORLEVEL% NEQ 0 (
    echo FAILED at core libs
    EXIT /B %ERRORLEVEL%
)

echo [2/3] Building all crates
cargo build --manifest-path C:\Users\84342\dc-build\Cargo.toml -p deepcoder-cli
IF %ERRORLEVEL% NEQ 0 (
    echo FAILED at CLI build
    EXIT /B %ERRORLEVEL%
)

echo [3/3] Building release binary
cargo build --manifest-path C:\Users\84342\dc-build\Cargo.toml --release -p deepcoder-cli
IF %ERRORLEVEL% NEQ 0 (
    echo FAILED at release build
    EXIT /B %ERRORLEVEL%
)

echo BUILD SUCCESS
echo Binary: C:\Users\84342\dc-build\target\release\deepcoder.exe
