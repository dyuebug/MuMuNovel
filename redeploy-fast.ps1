[CmdletBinding()]
param(
    [switch]$BuildFrontend,
    [switch]$SkipFrontendBuild,
    [switch]$NoCache,
    [switch]$UseCnMirror,
    [switch]$SkipSmoke,
    [switch]$NoPause,
    [switch]$NonInteractive,
    [int]$HealthTimeoutSec = 120
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location -Path $PSScriptRoot

function Test-InteractiveConsole {
    if (
        $NoPause -or
        $NonInteractive -or
        -not [Environment]::UserInteractive -or
        -not [string]::IsNullOrWhiteSpace($env:CI)
    ) {
        return $false
    }

    try {
        return (
            $Host.Name -eq 'ConsoleHost' -and
            -not [Console]::IsInputRedirected -and
            -not [Console]::IsOutputRedirected -and
            -not [Console]::IsErrorRedirected
        )
    } catch {
        return $false
    }
}

$ShouldPauseOnExit = Test-InteractiveConsole

$ComposeFile = "docker-compose.strangler.yml"
$DbService = "postgres"
$DbContainer = "mumunovel-postgres-new"
$DbMigratorService = "db-migrator"
$DbMigratorRunContainer = "mumunovel-db-migrator-once"
$RustService = "rust-backend"
$RustContainer = "mumunovel-rust"
$NginxService = "nginx"
$NginxContainer = "mumunovel-nginx"
$LogDirectory = Join-Path $PSScriptRoot "logs\ops"
$LogFilePath = Join-Path $LogDirectory "redeploy-fast.log"
$GatewaySmokeScript = Join-Path $PSScriptRoot "backend\tools\run_strangler_gateway_smoke.py"
$GatewaySmokeOutputPath = Join-Path $PSScriptRoot "tmp\smoke\tmp_strangler_gateway_smoke_latest.json"
$GatewaySmokeManifestPath = Join-Path $PSScriptRoot "deploy\strangler-gateway-probes.json"
$FrontendDirectory = Join-Path $PSScriptRoot "frontend"
$Utf8NoBomEncoding = [System.Text.UTF8Encoding]::new($false)

function Initialize-LogFile {
    New-Item -ItemType Directory -Force -Path $LogDirectory | Out-Null
    $header = @(
        "=== Fast redeploy started $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') ===",
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
        throw "Command failed with exit code ${exitCode}: $display"
    }
    return [pscustomobject]@{ ExitCode = $exitCode; Output = $outputText; DurationMs = $durationMs }
}

function Get-DockerComposeArgs {
    return @('-f', $ComposeFile)
}

function Get-DotEnvValue {
    param([string]$Name)

    $dotEnvPath = Join-Path $PSScriptRoot ".env"
    if (-not (Test-Path -Path $dotEnvPath)) { return $null }

    foreach ($line in [System.IO.File]::ReadLines($dotEnvPath)) {
        $trimmed = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($trimmed) -or $trimmed.StartsWith('#')) { continue }
        $match = [regex]::Match($trimmed, '^(?:export\s+)?(?<key>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?<value>.*)$')
        if (-not $match.Success -or $match.Groups['key'].Value -ne $Name) { continue }
        $value = $match.Groups['value'].Value.Trim()
        if ($value.Length -ge 2) {
            $first = $value.Substring(0, 1)
            $last = $value.Substring($value.Length - 1, 1)
            if (($first -eq '"' -and $last -eq '"') -or ($first -eq "'" -and $last -eq "'")) {
                $value = $value.Substring(1, $value.Length - 2)
            }
        }
        return $value
    }
    return $null
}

function Get-ComposeEnvironmentValue {
    param(
        [string]$Name,
        [string]$Default = $null
    )

    $value = [Environment]::GetEnvironmentVariable($Name)
    if (-not [string]::IsNullOrWhiteSpace($value)) { return $value }
    $value = Get-DotEnvValue -Name $Name
    if (-not [string]::IsNullOrWhiteSpace($value)) { return $value }
    return $Default
}

function Test-RequiredEnvironment {
    $jwtSecret = Get-ComposeEnvironmentValue -Name 'JWT_SECRET'
    if ([string]::IsNullOrWhiteSpace($jwtSecret)) {
        throw "JWT_SECRET is required for fast redeploy."
    }
    if ($jwtSecret.Length -lt 32) {
        throw "JWT_SECRET is set but too short. Use at least 32 characters."
    }
}

function Test-DockerDaemon {
    $result = Invoke-CommandLineCapture -CommandLine 'docker info'
    if ($result.ExitCode -eq 0) {
        return
    }
    throw "Docker daemon is not running.`n$($result.Output)"
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
        Start-Sleep -Seconds 2
    }
    throw "Timed out waiting for $Label. Last status: $lastStatus"
}

function Show-ContainerDiagnostics {
    param(
        [string]$ContainerName,
        [string]$ServiceName,
        [string]$Label,
        [int]$Tail = 80
    )
    Invoke-LoggedCommand -Command @('docker','inspect',$ContainerName) -Label "diag inspect $ServiceName" -IgnoreExitCode | Out-Null
    Invoke-LoggedCommand -Command (@('docker','compose') + (Get-DockerComposeArgs) + @('logs',"--tail=$Tail",$ServiceName)) -Label "diag logs $ServiceName" -IgnoreExitCode | Out-Null
}

function Build-ServiceImage {
    param([string]$ServiceName)
    $buildArgs = @('compose') + (Get-DockerComposeArgs) + @('build')
    if ($NoCache) { $buildArgs += '--no-cache' }
    if ($UseCnMirror) {
        $buildArgs += '--build-arg'; $buildArgs += 'USE_CN_MIRROR=true'
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

function Build-FrontendAssets {
    if (-not (Test-Path -Path $FrontendDirectory)) {
        throw "frontend directory not found: $FrontendDirectory"
    }
    $npm = if (Get-Command npm -ErrorAction SilentlyContinue) {
        'npm'
    } elseif (Get-Command pnpm -ErrorAction SilentlyContinue) {
        'pnpm'
    } else {
        throw 'Neither npm nor pnpm was found on PATH.'
    }

    Push-Location $FrontendDirectory
    try {
        if ($npm -eq 'npm') {
            Invoke-LoggedCommand -Command @($npm, 'run', 'build') -Label 'frontend build' | Out-Null
        } else {
            Invoke-LoggedCommand -Command @($npm, 'build') -Label 'frontend build' | Out-Null
        }
    }
    finally {
        Pop-Location
    }
}

function Get-ChangedPathList {
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        Write-LogLine "git command not found; frontend change detection disabled."
        return @()
    }

    $captured = @()
    & git -C $PSScriptRoot status --porcelain=v1 | Tee-Object -Variable captured | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-LogLine "git status failed; frontend change detection disabled."
        return @()
    }

    $paths = @()
    foreach ($line in $captured) {
        if ([string]::IsNullOrWhiteSpace($line) -or $line.Length -lt 4) { continue }
        $path = $line.Substring(3).Trim()
        if ($path.Contains(" -> ")) {
            $path = ($path -split " -> ", 2)[1].Trim()
        }
        $paths += ($path -replace '\\', '/')
    }
    return $paths
}

function Resolve-FrontendBuildDecision {
    if ($BuildFrontend -and $SkipFrontendBuild) {
        throw "Use either -BuildFrontend or -SkipFrontendBuild, not both."
    }

    if ($BuildFrontend) {
        return [pscustomobject]@{
            ShouldBuild = $true
            Reason = "forced by -BuildFrontend"
        }
    }

    if ($SkipFrontendBuild) {
        return [pscustomobject]@{
            ShouldBuild = $false
            Reason = "forced by -SkipFrontendBuild"
        }
    }

    $changedPaths = Get-ChangedPathList
    $frontendChangedPaths = @($changedPaths | Where-Object {
        $_ -like "frontend/*" -or $_ -like "backend/static/*"
    })

    if ($frontendChangedPaths.Count -gt 0) {
        return [pscustomobject]@{
            ShouldBuild = $true
            Reason = "detected changed frontend/static path: $($frontendChangedPaths[0])"
            DetectedPaths = $frontendChangedPaths
        }
    }

    return [pscustomobject]@{
        ShouldBuild = $false
        Reason = "no changed frontend/static paths detected"
        DetectedPaths = @()
    }
}

function Confirm-FrontendBuild {
    param([string[]]$DetectedPaths)

    Write-Host ""
    Write-Host "Detected frontend/static changes:" -ForegroundColor Yellow
    foreach ($path in ($DetectedPaths | Select-Object -First 8)) {
        Write-Host "  $path" -ForegroundColor DarkYellow
    }
    if ($DetectedPaths.Count -gt 8) {
        Write-Host "  ... and $($DetectedPaths.Count - 8) more" -ForegroundColor DarkYellow
    }
    Write-Host "Build frontend assets before fast redeploy? [Y/n/c] (Enter = Y)" -ForegroundColor Yellow

    while ($true) {
        $answer = (Read-Host).Trim().ToLowerInvariant()
        if ([string]::IsNullOrWhiteSpace($answer) -or $answer -in @('y', 'yes')) {
            return $true
        }
        if ($answer -in @('n', 'no')) {
            return $false
        }
        if ($answer -in @('c', 'cancel', 'q', 'quit')) {
            throw "Fast redeploy cancelled by user."
        }
        Write-Host "Please answer Y, N, or C." -ForegroundColor Yellow
    }
}

function Write-ExecutionPlan {
    param([object]$FrontendDecision)

    Write-Host ""
    Write-Host "Fast redeploy plan" -ForegroundColor Cyan
    Write-Host "  Frontend build : $($FrontendDecision.ShouldBuild) ($($FrontendDecision.Reason))" -ForegroundColor DarkCyan
    Write-Host "  Rust image     : rebuild rust-backend" -ForegroundColor DarkCyan
    Write-Host "  Database       : keep existing data; run Rust migration executor" -ForegroundColor DarkCyan
    Write-Host "  Containers     : recreate rust-backend, recreate nginx" -ForegroundColor DarkCyan
    Write-Host "  Gateway smoke  : $(-not $SkipSmoke)" -ForegroundColor DarkCyan
    Write-LogLine "Fast redeploy plan: frontend=$($FrontendDecision.ShouldBuild); rust=rebuild; migration=rust-db-migrator; containers=rust-backend,nginx; smoke=$(-not $SkipSmoke)"
}

function Invoke-DatabaseMigrator {
    Invoke-LoggedCommand -Command @('docker', 'rm', '-f', $DbMigratorRunContainer) -Label 'cleanup previous db migrator run' -IgnoreExitCode | Out-Null

    $command = @('docker', 'compose') + (Get-DockerComposeArgs) + @(
        'run',
        '--name', $DbMigratorRunContainer,
        '--no-deps',
        '-T',
        $DbMigratorService
    )

    try {
        return Invoke-LoggedCommand -Command $command -Label 'docker compose run db-migrator (Rust migration-executor)'
    }
    finally {
        Invoke-LoggedCommand -Command @('docker', 'rm', '-f', $DbMigratorRunContainer) -Label 'cleanup db migrator run' -IgnoreExitCode | Out-Null
    }
}

function Invoke-GatewaySmoke {
    if (-not (Test-Path -Path $GatewaySmokeScript)) {
        throw "Gateway smoke script not found: $GatewaySmokeScript"
    }
    if (-not (Test-Path -Path $GatewaySmokeManifestPath)) {
        throw "Gateway smoke manifest not found: $GatewaySmokeManifestPath"
    }

    $pythonLauncher = if (Get-Command python -ErrorAction SilentlyContinue) {
        'python'
    } elseif (Get-Command py -ErrorAction SilentlyContinue) {
        'py'
    } else {
        throw 'Neither python nor py launcher was found on PATH.'
    }

    $command = @(
        $pythonLauncher,
        '-X', 'utf8',
        $GatewaySmokeScript,
        '--base-url', 'http://localhost:8005',
        '--manifest', $GatewaySmokeManifestPath,
        '--profile', 'deploy',
        '--http-timeout', '10',
        '--output', $GatewaySmokeOutputPath
    )
    return Invoke-LoggedCommand -Command $command -Label 'gateway smoke probes'
}

Initialize-LogFile

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    throw "docker command not found. Please install Docker Desktop."
}

if (-not (Test-Path -Path $ComposeFile)) {
    throw "$ComposeFile not found in: $PSScriptRoot"
}

Write-Step "Checking required environment"
Test-RequiredEnvironment

Write-Step "Checking Docker daemon"
Test-DockerDaemon

$frontendBuildDecision = Resolve-FrontendBuildDecision
Write-Host "Frontend build: $($frontendBuildDecision.ShouldBuild) ($($frontendBuildDecision.Reason))" -ForegroundColor DarkCyan
Write-LogLine "Frontend build decision: $($frontendBuildDecision.ShouldBuild) ($($frontendBuildDecision.Reason))"

if (
    $frontendBuildDecision.ShouldBuild -and
    $frontendBuildDecision.Reason.StartsWith("detected changed frontend/static path:") -and
    -not $BuildFrontend -and
    -not $SkipFrontendBuild -and
    $ShouldPauseOnExit
) {
    $interactiveFrontendBuild = Confirm-FrontendBuild -DetectedPaths $frontendBuildDecision.DetectedPaths
    $frontendBuildDecision = [pscustomobject]@{
        ShouldBuild = $interactiveFrontendBuild
        Reason = if ($interactiveFrontendBuild) {
            $frontendBuildDecision.Reason + " (confirmed interactively)"
        } else {
            "frontend build skipped interactively"
        }
        DetectedPaths = $frontendBuildDecision.DetectedPaths
    }
    Write-Host "Frontend build: $($frontendBuildDecision.ShouldBuild) ($($frontendBuildDecision.Reason))" -ForegroundColor DarkCyan
    Write-LogLine "Frontend build decision after prompt: $($frontendBuildDecision.ShouldBuild) ($($frontendBuildDecision.Reason))"
}

Write-ExecutionPlan -FrontendDecision $frontendBuildDecision

if ($frontendBuildDecision.ShouldBuild) {
    Write-Step "Building frontend assets"
    Build-FrontendAssets
}

Write-Step "Building Rust backend image"
Build-ServiceImage -ServiceName $RustService

Write-Step "Ensuring PostgreSQL is running"
Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('up', '-d', $DbService)) -Label "docker compose up postgres" | Out-Null
Wait-ContainerHealthy -ContainerName $DbContainer -Label "PostgreSQL" -TimeoutSec 60

Write-Step "Running explicit database migration step"
try {
    Invoke-DatabaseMigrator | Out-Null
}
catch {
    Write-Host "Database migration step failed. Dumping diagnostics..." -ForegroundColor Yellow
    Write-LogLine "Database migration step failed: $($_.Exception.Message)"
    Show-ContainerDiagnostics -ContainerName $DbMigratorRunContainer -ServiceName $DbMigratorService -Label "Database migrator" -Tail 120
    Show-ContainerDiagnostics -ContainerName $DbContainer -ServiceName $DbService -Label "PostgreSQL" -Tail 120
    throw
}

Write-Step "Recreating Rust backend"
Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('up', '-d', '--no-deps', '--force-recreate', $RustService)) -Label "docker compose up rust-backend" | Out-Null
Wait-ContainerHealthy -ContainerName $RustContainer -Label "Rust backend" -TimeoutSec 60

Write-Step "Recreating Nginx gateway"
Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('up', '-d', '--no-deps', '--force-recreate', $NginxService)) -Label "docker compose up nginx" | Out-Null
Wait-ContainerHealthy -ContainerName $NginxContainer -Label "Nginx" -TimeoutSec 30

Write-Step "Container status"
Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('ps')) -Label "docker compose ps" | Out-Null

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
    } catch {
        $lastError = $_.Exception.Message
        Write-LogLine "Health probe: $lastError"
    }
    Start-Sleep -Seconds 2
}

if (-not $isHealthy) {
    Write-Host ""
    Write-Host "Health check failed. Dumping diagnostics..." -ForegroundColor Yellow
    Write-LogLine "Health check failed: $lastError"
    Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('ps')) -Label "diag ps" -IgnoreExitCode | Out-Null
    Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('logs', '--tail=80', $NginxService)) -Label "diag nginx logs" -IgnoreExitCode | Out-Null
    Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('logs', '--tail=40', $RustService)) -Label "diag rust logs" -IgnoreExitCode | Out-Null
    throw "Fast redeploy finished but health check failed: http://localhost:8005/health"
}

if (-not $SkipSmoke) {
    Write-Step "Running gateway smoke probes"
    try {
        Invoke-GatewaySmoke | Out-Null
        Write-Host "Gateway smoke OK. Summary: $GatewaySmokeOutputPath" -ForegroundColor Green
        Write-LogLine "Gateway smoke OK. Summary: $GatewaySmokeOutputPath"
    }
    catch {
        Write-Host "Gateway smoke failed. Dumping diagnostics..." -ForegroundColor Yellow
        Write-LogLine "Gateway smoke failed: $($_.Exception.Message)"
        if (Test-Path -Path $GatewaySmokeOutputPath) {
            try {
                $smokeSummary = [System.IO.File]::ReadAllText($GatewaySmokeOutputPath)
                Write-LogBlock "Gateway smoke summary:`n$smokeSummary"
            } catch {
                Write-LogLine "Failed to read gateway smoke summary: $($_.Exception.Message)"
            }
        }
        Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('ps')) -Label "diag ps after smoke" -IgnoreExitCode | Out-Null
        Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('logs', '--tail=80', $NginxService)) -Label "diag nginx logs after smoke" -IgnoreExitCode | Out-Null
        Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('logs', '--tail=40', $RustService)) -Label "diag rust logs after smoke" -IgnoreExitCode | Out-Null
        throw
    }
}

Write-Host ""
Write-Host "=== Fast Redeploy SUCCESS ===" -ForegroundColor Green
Write-Host "   Entry: http://localhost:8005" -ForegroundColor Green
Write-Host "   Rust:  docker://${RustContainer}:8001" -ForegroundColor DarkCyan
Write-Host "   DB:    docker://${DbContainer}:5432" -ForegroundColor DarkCyan
Write-LogLine "Fast redeploy succeeded."

if ($ShouldPauseOnExit) {
    Write-Host ""
    Write-Host "Deployment finished." -ForegroundColor Green
}
