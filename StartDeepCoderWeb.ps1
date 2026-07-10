param(
    [switch] $ValidateOnly,
    [switch] $NoOpen
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$Workspace = Join-Path $Root "deepcoder"
$WebDir = Join-Path $Root "deepcoder-web"
$TargetDir = "D:\deepcoder-target"
$TempDir = "D:\codex-temp"
$LogDir = Join-Path $Root "logs"
$WsAddr = if ($env:DEEPCODER_WS_ADDR) { $env:DEEPCODER_WS_ADDR } else { "127.0.0.1:8080" }
$WsPort = [int]($WsAddr.Split(":")[-1])
$WebPort = if ($env:DEEPCODER_WEB_PORT) { [int]$env:DEEPCODER_WEB_PORT } else { 5173 }
$WebUrl = "http://127.0.0.1:$WebPort"
$WsUrl = "ws://$WsAddr"

New-Item -ItemType Directory -Force -Path $TargetDir, $TempDir, $LogDir | Out-Null
$env:CARGO_TARGET_DIR = $TargetDir
$env:TEMP = $TempDir
$env:TMP = $TempDir

function Quote-PsLiteral {
    param([string] $Value)
    return "'" + $Value.Replace("'", "''") + "'"
}

function Test-TcpPort {
    param(
        [string] $HostName,
        [int] $Port
    )
    try {
        $client = [System.Net.Sockets.TcpClient]::new()
        $connect = $client.BeginConnect($HostName, $Port, $null, $null)
        if (-not $connect.AsyncWaitHandle.WaitOne(500)) {
            $client.Close()
            return $false
        }
        $client.EndConnect($connect)
        $client.Close()
        return $true
    } catch {
        return $false
    }
}

function Wait-TcpPort {
    param(
        [string] $HostName,
        [int] $Port,
        [int] $TimeoutSeconds
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-TcpPort -HostName $HostName -Port $Port) {
            return $true
        }
        Start-Sleep -Milliseconds 400
    }
    return $false
}

function Get-DeepCoderCli {
    $releaseExe = Join-Path $TargetDir "release\deepcoder.exe"
    $rootExe = Join-Path $Root "deepcoder.exe"
    if (Test-Path -LiteralPath $releaseExe) {
        return $releaseExe
    }

    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        if (Test-Path -LiteralPath $rootExe) {
            return $rootExe
        }
        throw "cargo was not found. Install Rust or build/copy deepcoder.exe first."
    }
    if (-not (Test-Path -LiteralPath $Workspace)) {
        throw "Rust workspace was not found: $Workspace"
    }

    Write-Host "Building DeepCoder CLI release binary..."
    Push-Location $Workspace
    try {
        cargo build --release -p deepcoder-cli
    } finally {
        Pop-Location
    }
    if (-not (Test-Path -LiteralPath $releaseExe)) {
        throw "Build finished but deepcoder.exe was not found: $releaseExe"
    }
    return $releaseExe
}

function Ensure-WebDeps {
    if (-not (Test-Path -LiteralPath $WebDir)) {
        throw "Web workspace was not found: $WebDir"
    }
    $npm = Get-Command npm -ErrorAction SilentlyContinue
    if (-not $npm) {
        throw "npm was not found. Install Node.js first."
    }
    if (-not (Test-Path -LiteralPath (Join-Path $WebDir "node_modules"))) {
        Write-Host "Installing web dependencies with npm ci..."
        Push-Location $WebDir
        try {
            npm ci
        } finally {
            Pop-Location
        }
    }
    return $npm.Source
}

function Start-HiddenPowerShell {
    param(
        [string] $Command,
        [string] $WorkingDirectory
    )
    Start-Process -FilePath "powershell.exe" `
        -WindowStyle Hidden `
        -WorkingDirectory $WorkingDirectory `
        -ArgumentList @("-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", $Command) | Out-Null
}

try {
    if ($ValidateOnly) {
        if (-not (Test-Path -LiteralPath $Workspace)) {
            throw "Rust workspace was not found: $Workspace"
        }
        if (-not (Test-Path -LiteralPath $WebDir)) {
            throw "Web workspace was not found: $WebDir"
        }
        if (-not (Test-Path -LiteralPath (Join-Path $WebDir "package.json"))) {
            throw "Web package.json was not found."
        }
        if (-not (Test-Path -LiteralPath (Join-Path $WebDir "package-lock.json"))) {
            throw "Web package-lock.json was not found."
        }
        Write-Host "DeepCoder Web launcher validation passed."
        exit 0
    }

    $cliExe = Get-DeepCoderCli
    $npmExe = Ensure-WebDeps
    $appLog = Join-Path $LogDir "deepcoder-app-server.log"
    $webLog = Join-Path $LogDir "deepcoder-web.log"

    if (-not (Test-TcpPort -HostName "127.0.0.1" -Port $WsPort)) {
        Write-Host "Starting DeepCoder AppServer on $WsAddr..."
        $cmd = "Set-Location -LiteralPath $(Quote-PsLiteral $Root); & $(Quote-PsLiteral $cliExe) app-server --ws $WsAddr *> $(Quote-PsLiteral $appLog)"
        Start-HiddenPowerShell -Command $cmd -WorkingDirectory $Root
        if (-not (Wait-TcpPort -HostName "127.0.0.1" -Port $WsPort -TimeoutSeconds 30)) {
            throw "AppServer did not become ready on $WsAddr. See log: $appLog"
        }
    } else {
        Write-Host "AppServer is already listening on $WsAddr."
    }

    if (-not (Test-TcpPort -HostName "127.0.0.1" -Port $WebPort)) {
        Write-Host "Starting DeepCoder Web on $WebUrl..."
        $webScript = if (Test-Path -LiteralPath (Join-Path $WebDir "dist\index.html")) { "preview" } else { "dev" }
        $cmd = "Set-Location -LiteralPath $(Quote-PsLiteral $WebDir); `$env:DEEPCODER_WS_PROXY_TARGET=$(Quote-PsLiteral $WsUrl); & $(Quote-PsLiteral $npmExe) run $webScript -- --host 127.0.0.1 --port $WebPort *> $(Quote-PsLiteral $webLog)"
        Start-HiddenPowerShell -Command $cmd -WorkingDirectory $WebDir
        if (-not (Wait-TcpPort -HostName "127.0.0.1" -Port $WebPort -TimeoutSeconds 30)) {
            throw "Web server did not become ready on $WebUrl. See log: $webLog"
        }
    } else {
        Write-Host "Web server is already listening on $WebUrl."
    }

    if (-not $NoOpen) {
        Write-Host "Opening $WebUrl"
        Start-Process $WebUrl
    } else {
        Write-Host "DeepCoder Web is ready: $WebUrl"
    }
    Write-Host "Logs:"
    Write-Host "  AppServer: $appLog"
    Write-Host "  Web:       $webLog"
    exit 0
} catch {
    Write-Host ""
    Write-Host "DeepCoder Web launch failed:" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    exit 1
}
