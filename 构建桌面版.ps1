$ErrorActionPreference = "Stop"

function Utf8Text {
    param([string] $Value)
    [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String($Value))
}

$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$Workspace = Join-Path $Root "deepcoder"
$ReleaseExe = "D:\deepcoder-target\release\deepcoder-desktop.exe"
$DesktopExeName = "DeepCoder$([char]0x684c)$([char]0x9762)$([char]0x7248).exe"
$RootExe = Join-Path $Root $DesktopExeName

New-Item -ItemType Directory -Force -Path "D:\deepcoder-target", "D:\codex-temp" | Out-Null
$env:CARGO_TARGET_DIR = "D:\deepcoder-target"
$env:TEMP = "D:\codex-temp"
$env:TMP = "D:\codex-temp"

if (-not (Test-Path -LiteralPath $Workspace)) {
    throw "$(Utf8Text '5rKh5pyJ5om+5YiwIFJ1c3Qgd29ya3NwYWNlOiA=')$Workspace"
}

Push-Location $Workspace
cargo build --release -p deepcoder-desktop
Pop-Location

if (-not (Test-Path -LiteralPath $ReleaseExe)) {
    throw "$(Utf8Text '5p6E5bu65a6M5oiQ5L2G5rKh5pyJ5om+5Yiw5Lqn54mpOiA=')$ReleaseExe"
}

Copy-Item -LiteralPath $ReleaseExe -Destination $RootExe -Force
Write-Host "$(Utf8Text '5qGM6Z2i54mI5bey5p6E5bu65bm25aSN5Yi25YiwOiA=')$RootExe"
