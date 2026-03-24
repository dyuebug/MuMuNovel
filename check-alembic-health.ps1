[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location -Path $PSScriptRoot

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

$pythonCommand = Get-Command python -ErrorAction SilentlyContinue
if (-not $pythonCommand) {
    throw "python command not found. Please install Python 3 or add it to PATH before running the Alembic health check."
}

$scriptPath = Join-Path $PSScriptRoot "backend/tools/check_alembic_revision_health.py"
if (-not (Test-Path -Path $scriptPath)) {
    throw "Alembic revision health script not found: $scriptPath"
}

Write-Step "Checking Alembic revision health"
& $pythonCommand.Source $scriptPath
$exitCode = $LASTEXITCODE
if ($exitCode -ne 0) {
    throw "Alembic revision health check failed with exit code $exitCode."
}

Write-Host ""
Write-Host "Alembic revision health check passed." -ForegroundColor Green
