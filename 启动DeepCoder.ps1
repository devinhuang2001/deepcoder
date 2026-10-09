$ErrorActionPreference = "Stop"

function Utf8Text {
    param([string] $Value)
    [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String($Value))
}

$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$Workspace = Join-Path $Root "deepcoder"
$DesktopExeName = "DeepCoder$([char]0x684c)$([char]0x9762)$([char]0x7248).exe"
$RootExe = Join-Path $Root $DesktopExeName
$ReleaseExe = "D:\deepcoder-target\release\deepcoder-desktop.exe"

New-Item -ItemType Directory -Force -Path "D:\deepcoder-target", "D:\codex-temp" | Out-Null
$env:CARGO_TARGET_DIR = "D:\deepcoder-target"
$env:TEMP = "D:\codex-temp"
$env:TMP = "D:\codex-temp"

function Start-DeepCoderExe {
    param([string] $ExePath)
    Write-Host "$(Utf8Text '5ZCv5YqoIERlZXBDb2RlciDmoYzpnaLniYg6IA==')$ExePath"
    Start-Process -FilePath $ExePath -WorkingDirectory $Root
}

try {
    if (Test-Path -LiteralPath $RootExe) {
        Start-DeepCoderExe -ExePath $RootExe
        exit 0
    }

    if (Test-Path -LiteralPath $ReleaseExe) {
        Start-DeepCoderExe -ExePath $ReleaseExe
        exit 0
    }

    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        throw "$(Utf8Text '5rKh5pyJ5om+5YiwIGNhcmdv77yM5Lmf5rKh5pyJ5om+5Yiw5bey5p6E5bu655qEIERlZXBDb2RlcuahjOmdoueJiC5leGXjgILor7flhYjov5DooYwg5p6E5bu65qGM6Z2i54mILnBzMeOAgg==')"
    }

    if (-not (Test-Path -LiteralPath $Workspace)) {
        throw "$(Utf8Text '5rKh5pyJ5om+5YiwIFJ1c3Qgd29ya3NwYWNlOiA=')$Workspace"
    }

    Write-Host "$(Utf8Text '5pyq5om+5Yiw5bey5p6E5bu6IGV4Ze+8jOato+WcqOS9v+eUqCBjYXJnbyDlkK/liqggcmVsZWFzZSDmoYzpnaLniYguLi4=')"
    Push-Location $Workspace
    cargo run -p deepcoder-desktop --release
    Pop-Location
} catch {
    Write-Host ""
    Write-Host "$(Utf8Text 'RGVlcENvZGVyIOWQr+WKqOWksei0pTo=')" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    exit 1
}
