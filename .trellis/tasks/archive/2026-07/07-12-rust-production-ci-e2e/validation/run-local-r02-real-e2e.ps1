param(
    [string]$EvidenceDirectory = "",
    [int]$PostgresPort = 55440,
    [int]$AppPort = 8003
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../../../..")).Path
$backendDir = Join-Path $repoRoot "backend-rs"
$frontendDir = Join-Path $repoRoot "frontend"
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
if ([string]::IsNullOrWhiteSpace($EvidenceDirectory)) {
    $EvidenceDirectory = Join-Path $PSScriptRoot "r02-local-real-e2e-$timestamp"
} elseif (-not [System.IO.Path]::IsPathRooted($EvidenceDirectory)) {
    $EvidenceDirectory = Join-Path $repoRoot $EvidenceDirectory
}
$evidenceDir = [System.IO.Path]::GetFullPath($EvidenceDirectory)
New-Item -ItemType Directory -Path $evidenceDir -Force | Out-Null

$containerName = "mumu-r02-local-$timestamp"
$backendProcess = $null
$containerStarted = $false
$failure = $null
$validationPassed = $false
$startedAt = (Get-Date).ToUniversalTime().ToString("o")
$lifecycle = [ordered]@{
    schema_version = 1
    runtime_owner = "backend-rs"
    process_target = "backend-rs/target/debug/mumu-novel-backend.exe"
    cleanup_status = "not_started"
    termination_signal = $null
}

function Invoke-LoggedProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$ArgumentList,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][string]$StdoutPath,
        [Parameter(Mandatory = $true)][string]$StderrPath
    )

    $process = Start-Process -FilePath $FilePath `
        -ArgumentList $ArgumentList `
        -WorkingDirectory $WorkingDirectory `
        -RedirectStandardOutput $StdoutPath `
        -RedirectStandardError $StderrPath `
        -WindowStyle Hidden `
        -Wait `
        -PassThru
    if ($process.ExitCode -ne 0) {
        throw "$FilePath exited with code $($process.ExitCode). See $StdoutPath and $StderrPath"
    }
    return $process.ExitCode
}

function Wait-PostgresHealthy {
    param([string]$Name)
    for ($attempt = 1; $attempt -le 60; $attempt++) {
        $status = docker inspect --format '{{.State.Health.Status}}' $Name 2>$null
        if ($LASTEXITCODE -eq 0 -and $status.Trim() -eq "healthy") {
            return
        }
        Start-Sleep -Seconds 1
    }
    throw "PostgreSQL container did not become healthy within 60 seconds"
}

function Wait-Http200 {
    param(
        [string]$Url,
        [string]$BodyPath,
        [string]$StatusPath,
        [int]$Attempts = 60
    )

    for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
        try {
            $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 3
            [System.IO.File]::WriteAllText($BodyPath, $response.Content, [System.Text.UTF8Encoding]::new($false))
            [System.IO.File]::WriteAllText($StatusPath, [string]$response.StatusCode + "`n", [System.Text.UTF8Encoding]::new($false))
            if ($response.StatusCode -eq 200) {
                return
            }
        } catch {
            [System.IO.File]::WriteAllText($StatusPath, "unavailable`n", [System.Text.UTF8Encoding]::new($false))
        }
        Start-Sleep -Seconds 1
    }
    throw "$Url did not return HTTP 200 within $Attempts attempts"
}

try {
    $portsInUse = Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue |
        Where-Object { $_.LocalPort -in @($PostgresPort, $AppPort, 5175) }
    if ($portsInUse) {
        throw "Required local port is already in use: $($portsInUse.LocalPort -join ', ')"
    }

    $toolchain = [ordered]@{
        captured_at = (Get-Date).ToUniversalTime().ToString("o")
        docker = (docker version --format '{{.Server.Version}}').Trim()
        node = (node --version).Trim()
        npm = (npm --version).Trim()
        cargo = (cargo --version).Trim()
        rustc = (rustc --version).Trim()
    }
    $toolchain | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidenceDir "toolchain.json") -Encoding utf8NoBOM

    $containerId = docker run --rm -d `
        --name $containerName `
        -e POSTGRES_DB=mumu_e2e `
        -e POSTGRES_USER=mumu_e2e `
        -e POSTGRES_PASSWORD=mumu_e2e `
        -p "${PostgresPort}:5432" `
        --health-cmd "pg_isready -U mumu_e2e -d mumu_e2e" `
        --health-interval 1s `
        --health-timeout 5s `
        --health-retries 30 `
        postgres:18-alpine
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to start PostgreSQL container"
    }
    $containerStarted = $true
    [System.IO.File]::WriteAllText((Join-Path $evidenceDir "postgres-container-id.txt"), $containerId.Trim() + "`n", [System.Text.UTF8Encoding]::new($false))
    Wait-PostgresHealthy -Name $containerName

    $env:DATABASE_URL = "postgresql://mumu_e2e:mumu_e2e@127.0.0.1:$PostgresPort/mumu_e2e"
    $env:DEBUG = "false"
    $env:JWT_SECRET = "e2e-rust-runtime-secret"
    $env:ENABLE_STARTUP_SCHEMA_SYNC = "false"
    $env:APP_HOST = "127.0.0.1"
    $env:APP_PORT = [string]$AppPort
    $env:LOCAL_AUTH_ENABLED = "1"
    $env:LOCAL_AUTH_USERNAME = "admin"
    $env:LOCAL_AUTH_PASSWORD = "admin123"
    $env:STATIC_DIR = "../backend/static"

    $cargo = (Get-Command cargo).Source
    Invoke-LoggedProcess -FilePath $cargo `
        -ArgumentList @("run", "--locked", "--", "migration-executor") `
        -WorkingDirectory $backendDir `
        -StdoutPath (Join-Path $evidenceDir "migration-executor.json") `
        -StderrPath (Join-Path $evidenceDir "migration-executor-stderr.log") | Out-Null
    "0`n" | Set-Content -LiteralPath (Join-Path $evidenceDir "migration-executor-exit-code.txt") -Encoding utf8NoBOM

    Invoke-LoggedProcess -FilePath $cargo `
        -ArgumentList @("run", "--locked", "--", "release-readiness-preflight") `
        -WorkingDirectory $backendDir `
        -StdoutPath (Join-Path $evidenceDir "release-preflight.json") `
        -StderrPath (Join-Path $evidenceDir "release-preflight-stderr.log") | Out-Null
    "0`n" | Set-Content -LiteralPath (Join-Path $evidenceDir "release-preflight-exit-code.txt") -Encoding utf8NoBOM

    Invoke-LoggedProcess -FilePath $cargo `
        -ArgumentList @("build", "--locked") `
        -WorkingDirectory $backendDir `
        -StdoutPath (Join-Path $evidenceDir "cargo-build.log") `
        -StderrPath (Join-Path $evidenceDir "cargo-build-stderr.log") | Out-Null

    $backendExecutable = Join-Path $backendDir "target/debug/mumu-novel-backend.exe"
    $backendProcess = Start-Process -FilePath $backendExecutable `
        -WorkingDirectory $backendDir `
        -RedirectStandardOutput (Join-Path $evidenceDir "rust-backend.log") `
        -RedirectStandardError (Join-Path $evidenceDir "rust-backend-stderr.log") `
        -WindowStyle Hidden `
        -PassThru
    $lifecycle.process_pid = $backendProcess.Id
    $lifecycle.cleanup_status = "running"
    [System.IO.File]::WriteAllText((Join-Path $evidenceDir "rust-backend.pid"), [string]$backendProcess.Id + "`n", [System.Text.UTF8Encoding]::new($false))

    Wait-Http200 `
        -Url "http://127.0.0.1:$AppPort/readyz" `
        -BodyPath (Join-Path $evidenceDir "readyz.json") `
        -StatusPath (Join-Path $evidenceDir "readyz-http-status.txt")
    Wait-Http200 `
        -Url "http://127.0.0.1:$AppPort/releasez" `
        -BodyPath (Join-Path $evidenceDir "releasez.json") `
        -StatusPath (Join-Path $evidenceDir "releasez-http-status.txt")

    $env:CI = "1"
    $env:E2E_REAL_BACKEND = "1"
    $npm = (Get-Command npm.cmd).Source
    Invoke-LoggedProcess -FilePath $npm `
        -ArgumentList @("run", "e2e", "--", "e2e/auth.spec.ts", "e2e/background-task-pages.spec.ts") `
        -WorkingDirectory $frontendDir `
        -StdoutPath (Join-Path $evidenceDir "playwright-smoke.log") `
        -StderrPath (Join-Path $evidenceDir "playwright-smoke-stderr.log") | Out-Null

    $validationPassed = $true
} catch {
    $failure = $_
    [ordered]@{
        schema_version = 1
        evidence_status = "failed"
        evidence_scope = "local-r02-real-e2e"
        error = $_.Exception.Message
        failed_at = (Get-Date).ToUniversalTime().ToString("o")
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidenceDir "local-r02-failure.json") -Encoding utf8NoBOM
} finally {
    if ($null -ne $backendProcess) {
        try {
            if (-not $backendProcess.HasExited) {
                Stop-Process -Id $backendProcess.Id -ErrorAction Stop
                if (-not $backendProcess.WaitForExit(10000)) {
                    Stop-Process -Id $backendProcess.Id -Force -ErrorAction Stop
                    $lifecycle.cleanup_status = "forced_kill"
                    $lifecycle.termination_signal = "TerminateProcess-Force"
                } else {
                    $lifecycle.cleanup_status = "terminated"
                    $lifecycle.termination_signal = "TerminateProcess"
                }
            } else {
                $lifecycle.cleanup_status = "already_exited"
            }
        } catch {
            $lifecycle.cleanup_status = "cleanup_failed"
            $lifecycle.cleanup_error = $_.Exception.Message
            if ($null -eq $failure) {
                $failure = $_
            }
        }
    }
    $lifecycle | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidenceDir "rust-backend-lifecycle.json") -Encoding utf8NoBOM

    if ($containerStarted) {
        docker logs $containerName *> (Join-Path $evidenceDir "postgres.log")
        docker stop $containerName *> (Join-Path $evidenceDir "postgres-stop.log")
    }
}

if ($null -ne $failure -or -not $validationPassed) {
    $failureMessage = if ($null -ne $failure) { $failure.Exception.Message } else { "Validation did not complete" }
    [ordered]@{
        schema_version = 1
        evidence_status = "failed"
        evidence_scope = "local-r02-real-e2e"
        error = $failureMessage
        failed_at = (Get-Date).ToUniversalTime().ToString("o")
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidenceDir "local-r02-failure.json") -Encoding utf8NoBOM
    Write-Error $failureMessage
    exit 1
}

$summary = [ordered]@{
    schema_version = 1
    evidence_status = "passed"
    evidence_scope = "local-r02-real-e2e"
    runtime_owner = "backend-rs"
    database = "postgresql-18-alpine"
    database_port = $PostgresPort
    app_port = $AppPort
    migration_executor = "passed"
    release_readiness_preflight = "passed"
    readyz = "passed"
    releasez = "passed"
    playwright_auth = "passed"
    playwright_background_task_pages = "passed"
    backend_lifecycle = $lifecycle.cleanup_status
    started_at = $startedAt
    completed_at = (Get-Date).ToUniversalTime().ToString("o")
}
$summary | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidenceDir "local-r02-success.json") -Encoding utf8NoBOM
Write-Output "R0.2 local real E2E passed. Evidence: $evidenceDir"
