$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$LauncherName = "$([char]0x542f)$([char]0x52a8)DeepCoder.ps1"
$LauncherPath = Join-Path $Root $LauncherName

& $LauncherPath
exit $LASTEXITCODE
