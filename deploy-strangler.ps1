[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$NoCache,
    [switch]$UseCnMirror,
    [switch]$SkipFrontendBuild,
    [switch]$FullRestart,
    [int]$HealthTimeoutSec = 180
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location -Path $PSScriptRoot

$ComposeFile = "docker-compose.strangler.yml"
$DbService = "postgres"
$DbContainer = "mumunovel-postgres-new"
$PythonService = "python-backend"
$PythonContainer = "mumunovel-python"
$RustService = "rust-backend"
$RustContainer = "mumunovel-rust"
$NginxService = "nginx"
$NginxContainer = "mumunovel-nginx"
$LogDirectory = Join-Path $PSScriptRoot "logs\ops"
$LogFilePath = Join-Path $LogDirectory "deploy-strangler.log"
$Utf8NoBomEncoding = [System.Text.UTF8Encoding]::new($false)

# =============================================================================
# Logging
# =============================================================================
function Initialize-LogFile {
    New-Item -ItemType Directory -Force -Path $LogDirectory | Out-Null
    $header = @(
        "=== Strangler Deploy started $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') ===",
        "ComposeFile: $ComposeFile",
        "Workspace: $PSScriptRoot",
        ""
    ) -join [Environment]::NewLine
    [System.IO.File]::WriteAllText($LogFilePath, $header, $Utf8NoBomEncoding)
}

function Write-LogLine {
    param([string]$Message)
    $timestamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
    [System.IO.File]::AppendAllText($LogFilePath, "[$timestamp] $Message$([Environment]::NewLine)", $Utf8NoBomEncoding)
}

function Write-LogBlock {
    param([string]$Message)
    if ([string]::IsNullOrWhiteSpace($Message)) { return }
    $timestamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
    $normalized = $Message -replace "`r?`n", [Environment]::NewLine
    [System.IO.File]::AppendAllText($LogFilePath, "[$timestamp] $normalized$([Environment]::NewLine)", $Utf8NoBomEncoding)
}

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
    Write-LogLine "==> $Message"
}

# =============================================================================
# Helpers
# =============================================================================
function Invoke-CommandLineCapture {
    param([string]$CommandLine)
    $output = & cmd.exe /d /c "$CommandLine 2>&1"
    return [pscustomobject]@{
        ExitCode = $LASTEXITCODE
        Output = ($output | Out-String).Trim()
    }
}

function Format-CommandDisplay {
    param([string[]]$Command)
    return ($Command | ForEach-Object {
        if ($_ -match '[\s"]') { '"' + ($_ -replace '"', '\"') + '"' }
        else { $_ }
    }) -join ' '
}

function Invoke-LoggedCommand {
    param(
        [string[]]$Command,
        [string]$Label,
        [switch]$IgnoreExitCode
    )
    if (-not $Command -or $Command.Count -eq 0) {
        throw 'Invoke-LoggedCommand requires at least one command segment.'
    }
    $display = Format-CommandDisplay -Command $Command
    Write-LogLine "Command start [$Label]: $display"
    $startedAt = Get-Date
    $captured = @()
    & cmd.exe /d /c "$display 2>&1" | Tee-Object -Variable captured | Out-Host
    $exitCode = if ($null -ne $LASTEXITCODE) { [int]$LASTEXITCODE } else { 0 }
    $durationMs = [int][Math]::Round(((Get-Date) - $startedAt).TotalMilliseconds)
    $outputText = ($captured | Out-String).Trim()
    if (-not [string]::IsNullOrWhiteSpace($outputText)) {
        Write-LogBlock $outputText
    }
    Write-LogLine "Command finish [$Label]: exit=$exitCode duration_ms=$durationMs"
    if (-not $IgnoreExitCode -and $exitCode -ne 0) {
        $hint = $null
        if ($Label -like "docker compose build*" -and ($outputText -match "deb\.debian\.org|security\.debian\.org|502\s+Bad Gateway|apt-get update")) {
            $hint = "Hint: Debian mirror failure during build. Retry with: deploy-strangler.bat -UseCnMirror"
        }
        if ($hint) { throw "Command failed: $display`n$hint" }
        throw "Command failed with exit code ${exitCode}: $display"
    }
    return [pscustomobject]@{ ExitCode = $exitCode; Output = $outputText; DurationMs = $durationMs }
}

function Get-DockerComposeArgs {
    $args = @('-f', $ComposeFile)
    return $args
}

function Wait-ContainerHealthy {
    param(
        [string]$ContainerName,
        [string]$Label,
        [int]$TimeoutSec = 120
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $lastStatus = $null
    while ((Get-Date) -lt $deadline) {
        try {
            $raw = docker inspect $ContainerName --format '{{json .State}}' 2>$null | Out-String
            $state = $raw | ConvertFrom-Json
            $status = if ($state.Health) { $state.Health.Status } else { $state.Status }
        } catch {
            $status = "inspect-failed"
        }
        if ($status -eq "healthy" -or $status -eq "running") {
            Write-Host "  $Label ready: $status" -ForegroundColor Green
            Write-LogLine "$Label ready: $status"
            return
        }
        if ($status -ne $lastStatus -and $status) {
            Write-Host "  ${Label}: $status..." -ForegroundColor DarkCyan
            $lastStatus = $status
        }
        Start-Sleep -Seconds 3
    }
    throw "Timed out waiting for $Label. Last status: $lastStatus"
}

function Test-DockerDaemon {
    $result = Invoke-CommandLineCapture -CommandLine 'docker info'
    if ($result.ExitCode -eq 0) {
        Write-LogLine "Docker daemon available."
        return
    }
    $errorMessage = @"
Docker daemon is not running.

请先启动 Docker Desktop，确认 Engine running 后再试。
原始错误: $($result.Output)
"@
    Write-LogBlock $errorMessage
    throw $errorMessage
}

function Build-ServiceImage {
    param(
        [string]$ServiceName,
        [string]$BuildContext = "."
    )
    $buildArgs = @('compose') + (Get-DockerComposeArgs) + @('build')
    if ($NoCache) { $buildArgs += '--no-cache' }
    if ($UseCnMirror) {
        $buildArgs += '--build-arg'; $buildArgs += 'USE_CN_MIRROR=true'
    }
    if ($SkipFrontendBuild) {
        $buildArgs += '--build-arg'; $buildArgs += 'SKIP_FRONTEND_BUILD=true'
    }
    foreach ($name in @('HTTP_PROXY','HTTPS_PROXY','NO_PROXY')) {
        $val = [Environment]::GetEnvironmentVariable($name)
        if ($val) {
            $buildArgs += '--build-arg'; $buildArgs += "${name}=$val"
            $buildArgs += '--build-arg'; $buildArgs += "$($name.ToLower())=$val"
        }
    }
    $buildArgs += $ServiceName
    $label = "docker compose build $ServiceName"
    if ($UseCnMirror) { $label += ' [cn-mirror]' }
    Invoke-LoggedCommand -Command (@('docker') + $buildArgs) -Label $label | Out-Null
}

# =============================================================================
# Main
# =============================================================================
Initialize-LogFile

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    throw "docker command not found. Please install Docker Desktop."
}

if (-not (Test-Path -Path $ComposeFile)) {
    $errorMessage = "$ComposeFile not found in: $PSScriptRoot"
    Write-LogBlock $errorMessage
    throw $errorMessage
}

Write-Step "Checking Docker daemon"
Test-DockerDaemon

# ---- Full restart ----
if ($FullRestart) {
    Write-Step "Stopping full strangler stack"
    Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('down')) -Label "docker compose down" | Out-Null
}
else {
    Write-Step "Keeping existing infra; only rebuilt services will be recreated"
}

# ---- Build ----
if (-not $SkipBuild) {
    Write-Step "Building Python backend image"
    Build-ServiceImage -ServiceName $PythonService

    Write-Step "Building Rust backend image"
    Build-ServiceImage -ServiceName $RustService
}

# ---- Start Postgres first ----
Write-Step "Starting PostgreSQL"
Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('up', '-d', $DbService)) -Label "docker compose up postgres" | Out-Null
Wait-ContainerHealthy -ContainerName $DbContainer -Label "PostgreSQL" -TimeoutSec 60

# ---- Start backends ----
Write-Step "Starting Python backend"
Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('up', '-d', '--force-recreate', $PythonService)) -Label "docker compose up python-backend" | Out-Null

Write-Step "Starting Rust backend"
Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('up', '-d', '--force-recreate', $RustService)) -Label "docker compose up rust-backend" | Out-Null

Write-Step "Waiting for backends to be healthy"
Wait-ContainerHealthy -ContainerName $PythonContainer -Label "Python backend" -TimeoutSec 120
Wait-ContainerHealthy -ContainerName $RustContainer -Label "Rust backend" -TimeoutSec 60

# ---- Start Nginx ----
Write-Step "Starting Nginx gateway"
Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('up', '-d', '--force-recreate', $NginxService)) -Label "docker compose up nginx" | Out-Null
Wait-ContainerHealthy -ContainerName $NginxContainer -Label "Nginx" -TimeoutSec 30

# ---- Status ----
Write-Step "Container status"
Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('ps')) -Label "docker compose ps" | Out-Null

# ---- Health check via Nginx ----
Write-Step "Health check via Nginx: http://localhost:8005/health"
$deadline = (Get-Date).AddSeconds($HealthTimeoutSec)
$isHealthy = $false
$lastError = $null

while ((Get-Date) -lt $deadline) {
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:8005/health" -UseBasicParsing -TimeoutSec 5
        if ($response.StatusCode -eq 200) {
            Write-Host "Health OK: $($response.Content)" -ForegroundColor Green
            Write-LogLine "Health OK: $($response.Content)"
            $isHealthy = $true
            break
        }
        $lastError = "HTTP $($response.StatusCode): $($response.Content)"
        Write-LogLine $lastError
    }
    catch {
        $lastError = $_.Exception.Message
        Write-LogLine "Health probe: $lastError"
    }
    Start-Sleep -Seconds 3
}

if (-not $isHealthy) {
    Write-Host ""
    Write-Host "Health check failed. Dumping diagnostics..." -ForegroundColor Yellow
    Write-LogLine "Health check failed: $lastError"
    Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('ps')) -Label "diag ps" -IgnoreExitCode | Out-Null
    Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('logs', '--tail=80', $NginxService)) -Label "diag nginx logs" -IgnoreExitCode | Out-Null
    Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('logs', '--tail=40', $RustService)) -Label "diag rust logs" -IgnoreExitCode | Out-Null
    Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('logs', '--tail=40', $PythonService)) -Label "diag python logs" -IgnoreExitCode | Out-Null
    throw "Strangler deploy finished but health check failed: http://localhost:8005/health"
}

# ---- Verify both backends through Nginx ----
Write-Step "Verifying Rust backend reachability via Nginx"
try {
    $rustResponse = Invoke-WebRequest -Uri "http://localhost:8005/api/admin/version" -UseBasicParsing -TimeoutSec 10
    Write-Host "Rust version endpoint: $($rustResponse.Content)" -ForegroundColor DarkCyan
    Write-LogLine "Rust backend reachable via Nginx: $($rustResponse.Content)"
}
catch {
    Write-Host "Warning: Rust backend reachability check failed: $($_.Exception.Message)" -ForegroundColor Yellow
    Write-LogLine "Rust backend reachability check failed: $($_.Exception.Message)"
}

Write-Step "Verifying Python backend reachability via Nginx"
try {
    $pythonResponse = Invoke-WebRequest -Uri "http://localhost:8005/memories/" -UseBasicParsing -TimeoutSec 10
    Write-Host "Python memories endpoint: HTTP $($pythonResponse.StatusCode)" -ForegroundColor DarkCyan
    Write-LogLine "Python backend reachable via Nginx: HTTP $($pythonResponse.StatusCode)"
}
catch {
    Write-Host "Warning: Python backend reachability check failed: $($_.Exception.Message)" -ForegroundColor Yellow
    Write-LogLine "Python backend reachability check failed: $($_.Exception.Message)"
}

Write-Host ""
Write-Host "=== Strangler Fig Deploy SUCCESS ===" -ForegroundColor Green
Write-Host "   入口: http://localhost:8005 (Nginx :8005)" -ForegroundColor Green
Write-Host "   Rust:  docker://$RustContainer:8001 (162 endpoints)" -ForegroundColor DarkCyan
Write-Host "   Python: docker://$PythonContainer:8000 (fallback)" -ForegroundColor DarkCyan
Write-Host "   DB:    docker://$DbContainer:5432" -ForegroundColor DarkCyan
Write-LogLine "Strangler deploy succeeded."

Write-Host ""
Read-Host -Prompt "部署完成，按 Enter 关闭窗口"