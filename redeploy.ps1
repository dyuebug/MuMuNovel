[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$SkipRecreate,
    [switch]$NoCache,
    [switch]$UseCnMirror,
    [switch]$SkipFrontendBuild,
    [switch]$SkipAssetVerification,
    [switch]$FullRestart,
    [switch]$NoPause,
    [switch]$NonInteractive,
    [switch]$RepairPostgresPassword,
    [int]$HealthTimeoutSec = 180,
    [string]$HealthUrl,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$RemainingArgs
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location -Path $PSScriptRoot

$DeployScript = Join-Path $PSScriptRoot "deploy-strangler.ps1"
if (-not (Test-Path -Path $DeployScript)) {
    throw "deploy-strangler.ps1 not found in: $PSScriptRoot"
}

$ignoredOptions = @()
if ($SkipRecreate) {
    $ignoredOptions += "-SkipRecreate"
}
if ($SkipAssetVerification) {
    $ignoredOptions += "-SkipAssetVerification"
}
if (-not [string]::IsNullOrWhiteSpace($HealthUrl)) {
    $ignoredOptions += "-HealthUrl"
}
if ($RemainingArgs -and $RemainingArgs.Count -gt 0) {
    $ignoredOptions += $RemainingArgs
}

if ($ignoredOptions.Count -gt 0) {
    Write-Warning (
        "redeploy.ps1 is a compatibility wrapper for deploy-strangler.ps1; " +
        "legacy option(s) ignored: " + ($ignoredOptions -join " ")
    )
}

$deployArgs = @()
if ($SkipBuild) {
    $deployArgs += "-SkipBuild"
}
if ($NoCache) {
    $deployArgs += "-NoCache"
}
if ($UseCnMirror) {
    $deployArgs += "-UseCnMirror"
}
if ($SkipFrontendBuild) {
    $deployArgs += "-SkipFrontendBuild"
}
if ($FullRestart) {
    $deployArgs += "-FullRestart"
}
if ($NoPause) {
    $deployArgs += "-NoPause"
}
if ($NonInteractive) {
    $deployArgs += "-NonInteractive"
}
if ($RepairPostgresPassword) {
    $deployArgs += "-RepairPostgresPassword"
}
$deployArgs += "-HealthTimeoutSec"
$deployArgs += $HealthTimeoutSec.ToString()

Write-Host "redeploy.ps1 now delegates to deploy-strangler.ps1 (Rust runtime + Rust db-migrator)." -ForegroundColor DarkCyan
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $DeployScript @deployArgs
exit $LASTEXITCODE
