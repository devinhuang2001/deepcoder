$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$LauncherPath = Join-Path $Root "StartDeepCoderWeb.ps1"

& $LauncherPath @args
exit $LASTEXITCODE
