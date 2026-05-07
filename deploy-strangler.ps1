[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$NoCache,
    [switch]$UseCnMirror,
    [switch]$SkipFrontendBuild,
    [switch]$FullRestart,
    [switch]$NoPause,
    [switch]$NonInteractive,
    [switch]$RepairPostgresPassword,
    [int]$HealthTimeoutSec = 180
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
        elseif ($Label -like "docker compose build*" -and ($outputText -match "docker\.m\.daocloud\.io|failed to resolve source metadata|401 Unauthorized|pull token")) {
            $hint = "Hint: Docker registry mirror/auth failure while pulling base images. Check Docker Desktop registry mirror settings, or retry after disabling the failing mirror."
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

function Test-RequiredStranglerEnvironment {
    $jwtSecret = Get-ComposeEnvironmentValue -Name 'JWT_SECRET'
    if ([string]::IsNullOrWhiteSpace($jwtSecret)) {
        $message = @"
JWT_SECRET is required for the strangler stack.

Add a stable secret to the repository .env file, for example:
JWT_SECRET=<random string with at least 32 characters>
"@
        Write-LogBlock $message
        throw $message
    }
    if ($jwtSecret.Length -lt 32) {
        $message = "JWT_SECRET is set but too short. Use at least 32 characters for a stable deployment secret."
        Write-LogBlock $message
        throw $message
    }

    $postgresPassword = Get-ComposeEnvironmentValue -Name 'POSTGRES_PASSWORD' -Default '123456'
    if ([string]::IsNullOrWhiteSpace($postgresPassword)) {
        $message = "POSTGRES_PASSWORD resolved to blank after applying compose defaults. Set POSTGRES_PASSWORD in .env to override it."
        Write-LogBlock $message
        throw $message
    }

    Write-LogLine "Required environment OK: JWT_SECRET is set and POSTGRES_PASSWORD resolved successfully."
}

function Invoke-PostgresNetworkCredentialProbe {
    param(
        [string]$PostgresUser,
        [string]$PostgresPassword,
        [string]$PostgresDb
    )

    $previousPgPassword = [Environment]::GetEnvironmentVariable('PGPASSWORD')
    [Environment]::SetEnvironmentVariable('PGPASSWORD', $PostgresPassword, 'Process')
    try {
        $command = @('docker', 'compose') + (Get-DockerComposeArgs) + @(
            'run', '--rm', '--no-deps', '-T', '--entrypoint', 'psql', '-e', 'PGPASSWORD',
            $PythonService,
            '-h', $DbService, '-U', $PostgresUser, '-d', $PostgresDb,
            '-v', 'ON_ERROR_STOP=1', '-c', 'select 1;'
        )
        return Invoke-LoggedCommand -Command $command -Label "verify postgres network credentials" -IgnoreExitCode
    }
    finally {
        if ($null -eq $previousPgPassword) {
            [Environment]::SetEnvironmentVariable('PGPASSWORD', $null, 'Process')
        }
        else {
            [Environment]::SetEnvironmentVariable('PGPASSWORD', $previousPgPassword, 'Process')
        }
    }
}

function ConvertTo-PostgresSqlLiteral {
    param([string]$Value)
    return "'" + ($Value -replace "'", "''") + "'"
}

function ConvertTo-PostgresIdentifier {
    param([string]$Value)
    return '"' + ($Value -replace '"', '""') + '"'
}

function Repair-PostgresPassword {
    param(
        [string]$PostgresUser,
        [string]$PostgresPassword,
        [string]$PostgresDb
    )

    $userIdentifier = ConvertTo-PostgresIdentifier -Value $PostgresUser
    $passwordLiteral = ConvertTo-PostgresSqlLiteral -Value $PostgresPassword
    $sql = "ALTER USER $userIdentifier WITH PASSWORD $passwordLiteral;"
    $command = @('docker', 'exec', $DbContainer, 'psql', '-U', $PostgresUser, '-d', $PostgresDb, '-v', 'ON_ERROR_STOP=1', '-c', $sql)
    Invoke-LoggedCommand -Command $command -Label "repair postgres password" | Out-Null
}

function Test-PostgresNetworkCredentials {
    $postgresUser = Get-ComposeEnvironmentValue -Name 'POSTGRES_USER' -Default 'mumuai'
    $postgresPassword = Get-ComposeEnvironmentValue -Name 'POSTGRES_PASSWORD' -Default '123456'
    $postgresDb = Get-ComposeEnvironmentValue -Name 'POSTGRES_DB' -Default 'mumuai_novel'
    if ([string]::IsNullOrWhiteSpace($postgresPassword)) {
        throw "POSTGRES_PASSWORD cannot be blank for the strangler stack."
    }

    $result = Invoke-PostgresNetworkCredentialProbe -PostgresUser $postgresUser -PostgresPassword $postgresPassword -PostgresDb $postgresDb
    if ($result.ExitCode -ne 0 -and $RepairPostgresPassword) {
        Write-Host "  PostgreSQL network credentials failed; repairing database user password..." -ForegroundColor Yellow
        Write-LogLine "PostgreSQL credential repair requested."
        Repair-PostgresPassword -PostgresUser $postgresUser -PostgresPassword $postgresPassword -PostgresDb $postgresDb
        $result = Invoke-PostgresNetworkCredentialProbe -PostgresUser $postgresUser -PostgresPassword $postgresPassword -PostgresDb $postgresDb
    }

    if ($result.ExitCode -ne 0) {
        $message = @(
            "PostgreSQL is healthy, but configured credentials cannot connect from python-backend over the Docker network.",
            "Current config:",
            "  POSTGRES_USER=$postgresUser",
            "  POSTGRES_DB=$postgresDb",
            "  POSTGRES_PASSWORD=<hidden>",
            "Restore the original POSTGRES_PASSWORD in .env, or rerun with -RepairPostgresPassword after confirming it is safe."
        ) -join [Environment]::NewLine
        Write-LogBlock $message
        throw $message
    }
}

function Show-ContainerDiagnostics {
    param(
        [string]$ContainerName,
        [string]$ServiceName,
        [string]$Label,
        [int]$Tail = 80
    )
    Write-LogLine "Collecting diagnostics for $Label ($ContainerName/$ServiceName)."
    Invoke-LoggedCommand -Command @('docker','inspect',$ContainerName) -Label "diag inspect $ServiceName" -IgnoreExitCode | Out-Null
    Invoke-LoggedCommand -Command (@('docker','compose') + (Get-DockerComposeArgs) + @('logs',"--tail=$Tail",$ServiceName)) -Label "diag logs $ServiceName" -IgnoreExitCode | Out-Null
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
    Write-LogLine "$Label health wait timed out. Last status: $lastStatus"
    if ($ContainerName -eq $PythonContainer) {
        Show-ContainerDiagnostics -ContainerName $PythonContainer -ServiceName $PythonService -Label $Label -Tail 120
    }
    elseif ($ContainerName -eq $RustContainer) {
        Show-ContainerDiagnostics -ContainerName $RustContainer -ServiceName $RustService -Label $Label -Tail 120
    }
    elseif ($ContainerName -eq $NginxContainer) {
        Show-ContainerDiagnostics -ContainerName $NginxContainer -ServiceName $NginxService -Label $Label -Tail 120
    }
    elseif ($ContainerName -eq $DbContainer) {
        Show-ContainerDiagnostics -ContainerName $DbContainer -ServiceName $DbService -Label $Label -Tail 120
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

Please make sure Docker Desktop is started and the Docker Engine is running.
Current docker info output: $($result.Output)
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

Write-Step "Checking required environment"
Test-RequiredStranglerEnvironment

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

Write-Step "Verifying PostgreSQL network credentials"
Test-PostgresNetworkCredentials

# ---- Start backends ----
Write-Step "Starting Python backend"
Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('up', '-d', '--force-recreate', $PythonService)) -Label "docker compose up python-backend" | Out-Null

Write-Step "Starting Rust backend"
Invoke-LoggedCommand -Command (@('docker', 'compose') + (Get-DockerComposeArgs) + @('up', '-d', '--force-recreate', $RustService)) -Label "docker compose up rust-backend" | Out-Null

Write-Step "Waiting for backends to be healthy"
Wait-ContainerHealthy -ContainerName $PythonContainer -Label "Python backend" -TimeoutSec 180
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
    $rustResponse = Invoke-WebRequest -Uri "http://localhost:8005/readyz" -UseBasicParsing -TimeoutSec 10
    Write-Host "Rust readiness endpoint: $($rustResponse.Content)" -ForegroundColor DarkCyan
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
Write-Host "   Entry:  http://localhost:8005 (Nginx :8005)" -ForegroundColor Green
Write-Host "   Rust:   docker://${RustContainer}:8001 (162 endpoints)" -ForegroundColor DarkCyan
Write-Host "   Python: docker://${PythonContainer}:8000 (fallback)" -ForegroundColor DarkCyan
Write-Host "   DB:     docker://${DbContainer}:5432" -ForegroundColor DarkCyan
Write-LogLine "Strangler deploy succeeded."

if ($ShouldPauseOnExit) {
    Write-Host ""
    Read-Host -Prompt "Deployment finished, press Enter to close this window"
}
