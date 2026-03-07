[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$SkipRecreate,
    [switch]$NoCache,
    [switch]$SkipAssetVerification,
    [int]$HealthTimeoutSec = 180,
    [string]$HealthUrl
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location -Path $PSScriptRoot

$AppService = "mumuainovel"
$AppContainerName = "mumuainovel-new"

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
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

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    throw "docker command not found. Please install and start Docker Desktop."
}

if (-not (Test-Path -Path "docker-compose.yml")) {
    throw "docker-compose.yml not found in: $PSScriptRoot"
}

if ([string]::IsNullOrWhiteSpace($HealthUrl)) {
    $appPort = Get-AppPort
    $HealthUrl = "http://localhost:$appPort/health"
}

Write-Step "Stopping existing containers"
docker compose down

$buildArgs = @("compose", "build")
if ($NoCache) {
    $buildArgs += "--no-cache"
}
$buildArgs += $AppService

if (-not $SkipBuild) {
    Write-Step "Building application image"
    docker @buildArgs
}

$upArgs = @("compose", "up", "-d")
if (-not $SkipRecreate) {
    $upArgs += "--force-recreate"
}
$upArgs += $AppService

Write-Step "Starting services"
docker @upArgs

Write-Step "Container status"
docker compose ps

Write-Step "Waiting for health check: $HealthUrl"
$deadline = (Get-Date).AddSeconds($HealthTimeoutSec)
$isHealthy = $false

while ((Get-Date) -lt $deadline) {
    try {
        $response = Invoke-WebRequest -Uri $HealthUrl -UseBasicParsing -TimeoutSec 5
        if ($response.StatusCode -eq 200) {
            Write-Host "Health response: $($response.Content)" -ForegroundColor Green
            $isHealthy = $true
            break
        }
    }
    catch {
    }
    Start-Sleep -Seconds 3
}

if (-not $isHealthy) {
    Write-Host ""
    Write-Host "Health check timed out. Printing recent logs..." -ForegroundColor Yellow
    docker compose logs --tail=120 mumuainovel
    throw "Redeploy finished but health check failed: $HealthUrl"
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
        }
        else {
            Write-Host "Container/workspace asset lookup unavailable, only verifying live asset exists." -ForegroundColor Yellow
        }
    }

    Write-Host "Live asset:      $liveAssetPath" -ForegroundColor DarkCyan
    if ($expectedAssetPath) {
        Write-Host "Expected asset ($expectedSource): $expectedAssetPath" -ForegroundColor DarkCyan
    }

    if ($expectedAssetPath -and $liveAssetPath -ne $expectedAssetPath) {
        throw "Redeploy finished but frontend asset mismatch detected. Live=$liveAssetPath Expected[$expectedSource]=$expectedAssetPath"
    }
}

Write-Host ""
Write-Host "Redeploy succeeded. App is reachable at: $HealthUrl" -ForegroundColor Green
