[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$SkipRecreate,
    [int]$HealthTimeoutSec = 180,
    [string]$HealthUrl
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location -Path $PSScriptRoot

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

$upArgs = @("compose", "up", "-d")
if (-not $SkipBuild) {
    $upArgs += "--build"
}
if (-not $SkipRecreate) {
    $upArgs += "--force-recreate"
}

Write-Step "Building and starting services"
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

Write-Host ""
Write-Host "Redeploy succeeded. App is reachable at: $HealthUrl" -ForegroundColor Green
