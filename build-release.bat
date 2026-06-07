@echo off
SETLOCAL

:: 用 MSVC + Windows SDK 编译
CALL "D:\VSCoding 2022\VC\Auxiliary\Build\vcvarsall.bat" x64
SET PATH=C:\Users\84342\.cargo\bin;%PATH%

echo [1/2] Building DeepCoder...
cargo build --manifest-path C:\Users\84342\dc-build\Cargo.toml --release -p deepcoder-cli

if %ERRORLEVEL% EQU 0 (
    echo ========================================
    echo BUILD SUCCESS
    echo Binary: C:\Users\84342\dc-build\target\release\deepcoder.exe
    echo ========================================

    copy /Y C:\Users\84342\dc-build\target\release\deepcoder.exe "D:\AAAA学习\笔记\agent学习\deepcoder\"
    echo Copied to D:\AAAA学习\笔记\agent学习\deepcoder\deepcoder.exe
) else (
    echo ========================================
    echo BUILD FAILED - need Windows SDK
    echo Run this first (as Admin):
    echo   "%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vs_installer.exe" ^
    echo     modify --installPath "D:\VSCoding 2022" ^
    echo     --add Microsoft.VisualStudio.Component.Windows10SDK.19041 --passive
    echo ========================================
)
pause
