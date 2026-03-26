[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$SkipRecreate,
    [switch]$NoCache,
    [switch]$UseCnMirror,
    [switch]$SkipFrontendBuild,
    [switch]$SkipAssetVerification,
    [switch]$FullRestart,
    [int]$HealthTimeoutSec = 180,
    [string]$HealthUrl
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location -Path $PSScriptRoot

$AppService = "mumunovel"
$AppContainerName = "mumunovel-new"
$DbService = "postgres"
$PostgresContainerName = "mumunovel-postgres-new"
$MigrationPreflightTimeoutSec = 90
$LogFilePath = Join-Path $PSScriptRoot "redeploy.log"
$Utf8NoBomEncoding = [System.Text.UTF8Encoding]::new($false)

function Initialize-LogFile {
    $header = @(
        "=== Redeploy started $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') ===",
        "Workspace: $PSScriptRoot",
        "CommandLine: $([Environment]::CommandLine)",
        ""
    ) -join [Environment]::NewLine

    [System.IO.File]::WriteAllText($LogFilePath, $header, $Utf8NoBomEncoding)
}

function Write-LogLine {
    param([string]$Message)

    $timestamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
    [System.IO.File]::AppendAllText(
        $LogFilePath,
        "[$timestamp] $Message$([Environment]::NewLine)",
        $Utf8NoBomEncoding
    )
}

function Write-LogBlock {
    param([string]$Message)

    if ([string]::IsNullOrWhiteSpace($Message)) {
        return
    }

    $timestamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
    $normalizedMessage = $Message -replace "`r?`n", [Environment]::NewLine
    [System.IO.File]::AppendAllText(
        $LogFilePath,
        "[$timestamp] $normalizedMessage$([Environment]::NewLine)",
        $Utf8NoBomEncoding
    )
}

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
    Write-LogLine "==> $Message"
}

function Get-RepoRelativePath {
    param([string]$Path)

    $repoRoot = [System.IO.Path]::GetFullPath($PSScriptRoot)
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if ($fullPath.StartsWith($repoRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $fullPath.Substring($repoRoot.Length).TrimStart('\', '/')
    }

    return $fullPath
}

function Test-AlembicRevisionIds {
    $scriptPath = Join-Path $PSScriptRoot "backend/tools/check_alembic_revision_health.py"
    if (-not (Test-Path -Path $scriptPath)) {
        $errorMessage = "Alembic revision health check script not found: $(Get-RepoRelativePath -Path $scriptPath)"
        Write-LogBlock $errorMessage
        throw $errorMessage
    }

    $pythonCommand = Get-Command python -ErrorAction SilentlyContinue
    if (-not $pythonCommand) {
        $errorMessage = "python command not found. Please install Python 3 or add it to PATH before running redeploy."
        Write-LogBlock $errorMessage
        throw $errorMessage
    }

    $commandOutput = & $pythonCommand.Source $scriptPath 2>&1
    $commandExitCode = $LASTEXITCODE
    $outputText = ($commandOutput | Out-String).Trim()

    if (-not [string]::IsNullOrWhiteSpace($outputText)) {
        Write-Host $outputText
        Write-LogBlock $outputText
    }

    if ($commandExitCode -ne 0) {
        $errorMessage = "Alembic revision health check failed. Run 'python backend/tools/check_alembic_revision_health.py' for details."
        Write-LogBlock $errorMessage
        throw $errorMessage
    }

    Write-LogLine "Alembic revision health check passed."
}

function Get-AppPort {
    if (-not (Test-Path -Path ".env")) {
        return "8000"
    }

    $appPortLine = Get-Content -Path ".env" | Where-Object { $_ -match '^\s*APP_PORT\s*=' } | Select-Object -First 1
    if (-not $appPortLine) {
        return "8000"
    }

    $value = (($appPortLine -split '=', 2)[1]).Trim()
    if ([string]::IsNullOrWhiteSpace($value)) {
        return "8000"
    }

    return $value
}

function Get-AppBaseUrl {
    $appPort = Get-AppPort
    return "http://localhost:$appPort"
}

function Get-LiveIndexAssetPath {
    param([string]$BaseUrl)

    $response = Invoke-WebRequest -Uri "$BaseUrl/" -UseBasicParsing -TimeoutSec 10
    $match = [regex]::Match($response.Content, '/assets/index-[^"'']+\.js')
    if (-not $match.Success) {
        throw "Unable to find live frontend entry asset from: $BaseUrl/"
    }

    return $match.Value
}

function Get-ContainerIndexAssetPath {
    $indexHtml = docker exec $AppContainerName sh -lc "cat /app/static/index.html"
    $indexHtml = ($indexHtml | Out-String)
    $match = [regex]::Match($indexHtml, '/assets/index-[^"'']+\.js')

    if (-not $match.Success) {
        throw "Unable to find frontend entry asset inside container: $AppContainerName"
    }

    return $match.Value
}

function Get-ContainerBackendStamp {
    $code = "import hashlib, pathlib, datetime; p=pathlib.Path('/app/app/api/background_tasks.py'); ts=datetime.datetime.utcfromtimestamp(p.stat().st_mtime).isoformat()+'Z'; h=hashlib.sha256(p.read_bytes()).hexdigest()[:12]; print(f'{ts} {h}')"
    $stamp = docker exec $AppContainerName python -c $code
    return ($stamp | Out-String).Trim()
}

function Get-LocalIndexAssetPath {
    $localIndexFile = Join-Path $PSScriptRoot "backend/static/index.html"
    if (-not (Test-Path -Path $localIndexFile)) {
        return $null
    }

    $indexHtml = Get-Content -Path $localIndexFile -Raw
    $match = [regex]::Match($indexHtml, '/assets/index-[^"'']+\.js')
    if (-not $match.Success) {
        return $null
    }

    return $match.Value
}

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
        if ($_ -match '[\s"]') {
            '"' + ($_ -replace '"', '\"') + '"'
        }
        else {
            $_
        }
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

    $commandDisplay = Format-CommandDisplay -Command $Command
    $commandName = $Command[0]
    $arguments = @()
    if ($Command.Count -gt 1) {
        $arguments = $Command[1..($Command.Count - 1)]
    }

    Write-LogLine("Command start [$Label]: $commandDisplay")
    $startedAt = Get-Date
    $captured = @()
    & cmd.exe /d /c "$commandDisplay 2>&1" | Tee-Object -Variable captured
    $exitCode = if ($null -ne $LASTEXITCODE) { [int]$LASTEXITCODE } else { 0 }
    $durationMs = [int][Math]::Round(((Get-Date) - $startedAt).TotalMilliseconds)
    $outputText = ($captured | Out-String).Trim()

    if (-not [string]::IsNullOrWhiteSpace($outputText)) {
        Write-LogBlock $outputText
    }

    Write-LogLine("Command finish [$Label]: exit=$exitCode duration_ms=$durationMs")
    if (-not $IgnoreExitCode -and $exitCode -ne 0) {
        throw "Command failed with exit code ${exitCode}: $commandDisplay"
    }

    return [pscustomobject]@{
        ExitCode = $exitCode
        Output = $outputText
        DurationMs = $durationMs
        CommandLine = $commandDisplay
    }
}

function Get-DockerContextSummary {
    try {
        $result = Invoke-CommandLineCapture -CommandLine 'docker context ls'
        if ([string]::IsNullOrWhiteSpace($result.Output)) {
            return "docker context ls returned no output."
        }

        return $result.Output
    }
    catch {
        return "docker context ls failed: $($_.Exception.Message)"
    }
}

function Get-DockerServiceSummary {
    try {
        $service = Get-Service -Name "com.docker.service" -ErrorAction SilentlyContinue
        if (-not $service) {
            return "com.docker.service not found."
        }

        return "com.docker.service: $($service.Status)"
    }
    catch {
        return "Unable to query com.docker.service: $($_.Exception.Message)"
    }
}

function Get-WslStatusSummary {
    if (-not (Get-Command wsl -ErrorAction SilentlyContinue)) {
        return "wsl command not found."
    }

    try {
        $result = Invoke-CommandLineCapture -CommandLine 'wsl -l -v'
        if ([string]::IsNullOrWhiteSpace($result.Output)) {
            return "wsl -l -v returned no output."
        }

        return $result.Output
    }
    catch {
        return "wsl -l -v failed: $($_.Exception.Message)"
    }
}

function Test-DockerDaemon {
    $dockerInfoResult = Invoke-CommandLineCapture -CommandLine 'docker info'
    if ($dockerInfoResult.ExitCode -eq 0) {
        Write-LogLine "Docker daemon is available."
        return
    }

    $details = $dockerInfoResult.Output
    $dockerContextSummary = Get-DockerContextSummary
    $dockerServiceSummary = Get-DockerServiceSummary
    $wslStatusSummary = Get-WslStatusSummary

    if ($details -match 'dockerDesktopLinuxEngine') {
        $errorMessage = @"
Docker daemon is not running.

请先启动 Docker Desktop，并确认：
1. Docker Desktop 状态为 Engine running
2. 当前使用的是 Linux containers
3. 如仍失败，可执行 `wsl --shutdown` 后重启 Docker Desktop

附加诊断：
- $dockerServiceSummary

docker context ls:
$dockerContextSummary

wsl -l -v:
$wslStatusSummary

原始错误：
$details
"@
        Write-LogBlock $errorMessage
        throw $errorMessage
    }

    $errorMessage = @"
Unable to connect to Docker daemon.

附加诊断：
- $dockerServiceSummary

docker context ls:
$dockerContextSummary

wsl -l -v:
$wslStatusSummary

原始错误：
$details
"@
    Write-LogBlock $errorMessage
    throw $errorMessage
}

function Wait-ContainerHealthy {
    param(
        [string]$ContainerName,
        [string]$Label,
        [int]$TimeoutSec = 90
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $lastStatus = $null
    while ((Get-Date) -lt $deadline) {
        $inspectResult = Invoke-LoggedCommand -Command @(
            "docker", "inspect", $ContainerName, "--format", "{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}"
        ) -Label "inspect $Label status" -IgnoreExitCode
        $status = ($inspectResult.Output | Out-String).Trim()
        if ($status -eq "healthy" -or $status -eq "running") {
            Write-LogLine "$Label is ready: $status"
            return
        }
        if (-not [string]::IsNullOrWhiteSpace($status) -and $status -ne $lastStatus) {
            Write-LogLine "$Label status: $status"
            $lastStatus = $status
        }
        Start-Sleep -Seconds 2
    }

    $errorMessage = "Timed out waiting for $Label to become healthy. Last status: $lastStatus"
    Write-LogBlock $errorMessage
    throw $errorMessage
}

function Invoke-AlembicVersionCapacityPreflight {
    Write-Step "Starting database service for Alembic preflight"
    Invoke-LoggedCommand -Command @("docker", "compose", "up", "-d", $DbService) -Label "docker compose up postgres" | Out-Null
    Wait-ContainerHealthy -ContainerName $PostgresContainerName -Label $DbService -TimeoutSec $MigrationPreflightTimeoutSec

    Write-Step "Ensuring Alembic version table capacity"
    Invoke-LoggedCommand -Command @(
        "docker", "compose", "run", "--rm", "--no-deps", "--entrypoint", "python", $AppService, "tools/ensure_alembic_version_table_capacity.py"
    ) -Label "docker compose run alembic capacity preflight" | Out-Null
}

Initialize-LogFile

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    $errorMessage = "docker command not found. Please install and start Docker Desktop."
    Write-LogBlock $errorMessage
    throw $errorMessage
}

if (-not (Test-Path -Path "docker-compose.yml")) {
    $errorMessage = "docker-compose.yml not found in: $PSScriptRoot"
    Write-LogBlock $errorMessage
    throw $errorMessage
}

Write-Step "Checking Alembic revision health"
Test-AlembicRevisionIds

Write-Step "Checking Docker daemon"
Test-DockerDaemon

if ([string]::IsNullOrWhiteSpace($HealthUrl)) {
    $appPort = Get-AppPort
    $HealthUrl = "http://localhost:$appPort/readyz"
}

if ($FullRestart) {
    Write-Step "Stopping full stack"
    Invoke-LoggedCommand -Command @("docker", "compose", "down") -Label "docker compose down" | Out-Null
}
else {
    Write-Step "Keeping dependent services running; only app container will be recreated"
}

$buildArgs = @("compose", "build")
if ($NoCache) {
    $buildArgs += "--no-cache"
}
if ($UseCnMirror) {
    $buildArgs += "--build-arg"
    $buildArgs += "USE_CN_MIRROR=true"
}
if ($SkipFrontendBuild) {
    $buildArgs += "--build-arg"
    $buildArgs += "SKIP_FRONTEND_BUILD=true"
}
$buildArgs += $AppService

if (-not $SkipBuild) {
    Write-Step "Building application image"
    Invoke-LoggedCommand -Command (@("docker") + $buildArgs) -Label "docker compose build" | Out-Null
}

Invoke-AlembicVersionCapacityPreflight

$upArgs = @("compose", "up", "-d", "--remove-orphans")
if (-not $SkipRecreate) {
    $upArgs += "--force-recreate"
}
$upArgs += $AppService

Write-Step "Starting application service"
Invoke-LoggedCommand -Command (@("docker") + $upArgs) -Label "docker compose up" | Out-Null

Write-Step "Container status"
Invoke-LoggedCommand -Command @("docker", "compose", "ps") -Label "docker compose ps" | Out-Null

Write-Step "Waiting for health check: $HealthUrl"
$deadline = (Get-Date).AddSeconds($HealthTimeoutSec)
$isHealthy = $false
$lastHealthError = $null
$lastHealthStatus = $null

while ((Get-Date) -lt $deadline) {
    try {
        $response = Invoke-WebRequest -Uri $HealthUrl -UseBasicParsing -TimeoutSec 5
        $lastHealthStatus = $response.StatusCode
        if ($response.StatusCode -eq 200) {
            Write-Host "Health response: $($response.Content)" -ForegroundColor Green
            Write-LogLine "Health response: $($response.Content)"
            $isHealthy = $true
            break
        }

        $lastHealthError = "Unexpected status code: $($response.StatusCode); body=$($response.Content)"
        Write-LogLine $lastHealthError
    }
    catch {
        $lastHealthError = $_.Exception.Message
        Write-LogLine "Health probe failed: $lastHealthError"
    }
    Start-Sleep -Seconds 3
}

if (-not $isHealthy) {
    Write-Host ""
    Write-Host "Health check timed out. Printing recent diagnostics..." -ForegroundColor Yellow
    Write-LogLine "Health check timed out. Printing recent diagnostics..."
    if ($lastHealthStatus) {
        Write-Host "Last health status: $lastHealthStatus" -ForegroundColor Yellow
        Write-LogLine "Last health status: $lastHealthStatus"
    }
    if ($lastHealthError) {
        Write-Host "Last health error: $lastHealthError" -ForegroundColor Yellow
        Write-LogLine "Last health error: $lastHealthError"
    }
    Invoke-LoggedCommand -Command @("docker", "compose", "ps") -Label "diagnostic docker compose ps" -IgnoreExitCode | Out-Null
    Invoke-LoggedCommand -Command @("docker", "inspect", $AppContainerName, "--format", "{{json .State.Health}}") -Label "diagnostic docker inspect health" -IgnoreExitCode | Out-Null
    Invoke-LoggedCommand -Command @("docker", "compose", "logs", "--tail=120", $AppService) -Label "diagnostic docker compose logs" -IgnoreExitCode | Out-Null
    $errorMessage = "Redeploy finished but health check failed: $HealthUrl"
    Write-LogBlock $errorMessage
    throw $errorMessage
}

Write-Step "Verifying backend build stamp"
try {
    $backendStamp = Get-ContainerBackendStamp
    if ($backendStamp) {
        Write-Host "Backend stamp (UTC + sha12): $backendStamp" -ForegroundColor DarkCyan
        Write-LogLine "Backend stamp (UTC + sha12): $backendStamp"
    }
    else {
        Write-Host "Backend stamp unavailable (empty response)." -ForegroundColor Yellow
        Write-LogLine "Backend stamp unavailable (empty response)."
    }
}
catch {
    Write-Host "Backend stamp lookup failed: $($_.Exception.Message)" -ForegroundColor Yellow
    Write-LogLine "Backend stamp lookup failed: $($_.Exception.Message)"
}

if (-not $SkipAssetVerification) {
    $baseUrl = Get-AppBaseUrl
    Write-Step "Verifying live frontend asset"

    $liveAssetPath = Get-LiveIndexAssetPath -BaseUrl $baseUrl
    $expectedAssetPath = $null
    $expectedSource = $null

    try {
        $expectedAssetPath = Get-ContainerIndexAssetPath
        $expectedSource = "container"
    }
    catch {
        $expectedAssetPath = Get-LocalIndexAssetPath
        if ($expectedAssetPath) {
            $expectedSource = "workspace"
            Write-Host "Container asset lookup failed, fallback to workspace static asset verification." -ForegroundColor Yellow
            Write-LogLine "Container asset lookup failed, fallback to workspace static asset verification."
        }
        else {
            Write-Host "Container/workspace asset lookup unavailable, only verifying live asset exists." -ForegroundColor Yellow
            Write-LogLine "Container/workspace asset lookup unavailable, only verifying live asset exists."
        }
    }

    Write-Host "Live asset:      $liveAssetPath" -ForegroundColor DarkCyan
    Write-LogLine "Live asset: $liveAssetPath"
    if ($expectedAssetPath) {
        Write-Host "Expected asset ($expectedSource): $expectedAssetPath" -ForegroundColor DarkCyan
        Write-LogLine "Expected asset ($expectedSource): $expectedAssetPath"
    }

    if ($expectedAssetPath -and $liveAssetPath -ne $expectedAssetPath) {
        $errorMessage = "Redeploy finished but frontend asset mismatch detected. Live=$liveAssetPath Expected[$expectedSource]=$expectedAssetPath"
        Write-LogBlock $errorMessage
        throw $errorMessage
    }
}

Write-Host ""
Write-Host "Redeploy succeeded. App is reachable at: $HealthUrl" -ForegroundColor Green
Write-LogLine "Redeploy succeeded. App is reachable at: $HealthUrl"
