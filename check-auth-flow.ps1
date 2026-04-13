[CmdletBinding()]
param(
    [int]$BackendPort,
    [int]$FrontendPort = 5175,
    [int]$DockerAppPort,
    [string]$BackendHost = "127.0.0.1",
    [string]$FrontendHost = "127.0.0.1",
    [string]$DockerHost = "127.0.0.1",
    [string]$Username,
    [string]$Password,
    [switch]$SkipFrontend,
    [switch]$SkipDockerReadyz
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location -Path $PSScriptRoot

$Utf8NoBomEncoding = [System.Text.UTF8Encoding]::new($false)

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Get-EnvValue {
    param(
        [string]$Key,
        [string]$Default = ""
    )

    $envPath = Join-Path $PSScriptRoot ".env"
    if (-not (Test-Path -Path $envPath)) {
        return $Default
    }

    $line = Get-Content -Path $envPath | Where-Object { $_ -match "^\s*$([regex]::Escape($Key))\s*=" } | Select-Object -First 1
    if (-not $line) {
        return $Default
    }

    $value = (($line -split '=', 2)[1]).Trim()
    if ([string]::IsNullOrWhiteSpace($value)) {
        return $Default
    }

    return $value.Trim('"')
}

function Ensure-RequiredTool {
    param([string]$CommandName)

    $command = Get-Command $CommandName -ErrorAction SilentlyContinue
    if (-not $command) {
        throw "Missing required command: $CommandName"
    }

    return $command.Source
}

function New-TempPath {
    param([string]$Suffix)

    $directory = Join-Path $PSScriptRoot "tmp"
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    return Join-Path $directory ([System.Guid]::NewGuid().ToString("N") + $Suffix)
}

function Invoke-CurlJson {
    param(
        [string]$Method,
        [string]$Url,
        [string]$CookieJar = "",
        [string]$BodyFile = "",
        [string]$ContentType = "application/json"
    )

    $headerPath = New-TempPath -Suffix ".headers.txt"
    $bodyPath = New-TempPath -Suffix ".body.txt"

    try {
        $args = @('-sS', '-D', $headerPath, '-o', $bodyPath, '-X', $Method)
        if (-not [string]::IsNullOrWhiteSpace($CookieJar)) {
            $args += @('-b', $CookieJar, '-c', $CookieJar)
        }
        if (-not [string]::IsNullOrWhiteSpace($BodyFile)) {
            $args += @('-H', "Content-Type: $ContentType", '--data-binary', "@$BodyFile")
        }
        $args += $Url

        & curl.exe @args
        $exitCode = $LASTEXITCODE
        if ($exitCode -ne 0) {
            throw "curl request failed with exit code ${exitCode}: $Method $Url"
        }

        $headerLines = Get-Content -Path $headerPath
        $statusLine = $headerLines | Where-Object { $_ -match '^HTTP/' } | Select-Object -Last 1
        if (-not $statusLine) {
            throw "Unable to parse HTTP status line for: $Method $Url"
        }

        $statusCode = [int](($statusLine -split ' ')[1])
        $bodyText = if (Test-Path -Path $bodyPath) {
            [System.IO.File]::ReadAllText($bodyPath)
        }
        else {
            ""
        }

        $jsonBody = $null
        if (-not [string]::IsNullOrWhiteSpace($bodyText)) {
            try {
                $jsonBody = $bodyText | ConvertFrom-Json
            }
            catch {
                $jsonBody = $null
            }
        }

        return [pscustomobject]@{
            Method = $Method
            Url = $Url
            StatusCode = $statusCode
            BodyText = $bodyText
            JsonBody = $jsonBody
        }
    }
    finally {
        Remove-Item -Path $headerPath, $bodyPath -Force -ErrorAction SilentlyContinue
    }
}

function Assert-StatusCode {
    param(
        [string]$Label,
        $Response,
        [int]$ExpectedStatusCode
    )

    if ($Response.StatusCode -ne $ExpectedStatusCode) {
        throw "$Label expected HTTP $ExpectedStatusCode but got $($Response.StatusCode). Body: $($Response.BodyText)"
    }

    Write-Host "[PASS] $Label -> HTTP $ExpectedStatusCode" -ForegroundColor Green
}

function Assert-JsonFieldEquals {
    param(
        [string]$Label,
        $Response,
        [string]$Field,
        [string]$ExpectedValue
    )

    if ($null -eq $Response.JsonBody) {
        throw "$Label expected JSON body, but response body is not valid JSON. Body: $($Response.BodyText)"
    }

    $actualValue = $Response.JsonBody.$Field
    if ($actualValue -ne $ExpectedValue) {
        throw "$Label expected $Field='$ExpectedValue' but got '$actualValue'. Body: $($Response.BodyText)"
    }

    Write-Host "[PASS] $Label -> $Field=$ExpectedValue" -ForegroundColor Green
}

function Assert-JsonFieldNotEmpty {
    param(
        [string]$Label,
        $Response,
        [string]$Field
    )

    if ($null -eq $Response.JsonBody) {
        throw "$Label expected JSON body, but response body is not valid JSON. Body: $($Response.BodyText)"
    }

    $actualValue = $Response.JsonBody.$Field
    if ([string]::IsNullOrWhiteSpace([string]$actualValue)) {
        throw "$Label expected non-empty field '$Field'. Body: $($Response.BodyText)"
    }

    Write-Host "[PASS] $Label -> $Field is populated" -ForegroundColor Green
}

function Test-Readyz {
    param(
        [string]$Label,
        [string]$BaseUrl
    )

    $response = Invoke-CurlJson -Method 'GET' -Url "$BaseUrl/readyz"
    Assert-StatusCode -Label "$Label readyz" -Response $response -ExpectedStatusCode 200

    if ($null -ne $response.JsonBody -and $response.JsonBody.status -ne 'ready') {
        throw "$Label readyz expected JSON status 'ready'. Body: $($response.BodyText)"
    }

    Write-Host "[PASS] $Label readyz -> status=ready" -ForegroundColor Green
}

function Test-AuthFlow {
    param(
        [string]$Label,
        [string]$BaseUrl,
        [string]$CookieJar,
        [string]$LoginBodyFile
    )

    if (Test-Path -Path $CookieJar) {
        Remove-Item -Path $CookieJar -Force -ErrorAction SilentlyContinue
    }

    $notLoggedInDetail = [string]([char]0x672A) + ([char]0x767B) + ([char]0x5F55)

    Write-Step "$Label auth config"
    $configResponse = Invoke-CurlJson -Method 'GET' -Url "$BaseUrl/api/auth/config"
    Assert-StatusCode -Label "$Label auth config" -Response $configResponse -ExpectedStatusCode 200

    Write-Step "$Label unauthenticated user"
    $unauthenticatedUser = Invoke-CurlJson -Method 'GET' -Url "$BaseUrl/api/auth/user" -CookieJar $CookieJar
    Assert-StatusCode -Label "$Label unauthenticated user" -Response $unauthenticatedUser -ExpectedStatusCode 401
    Assert-JsonFieldEquals -Label "$Label unauthenticated user" -Response $unauthenticatedUser -Field 'detail' -ExpectedValue $notLoggedInDetail

    Write-Step "$Label login"
    $loginResponse = Invoke-CurlJson -Method 'POST' -Url "$BaseUrl/api/auth/local/login" -CookieJar $CookieJar -BodyFile $LoginBodyFile
    Assert-StatusCode -Label "$Label login" -Response $loginResponse -ExpectedStatusCode 200
    if ($null -eq $loginResponse.JsonBody -or $loginResponse.JsonBody.success -ne $true) {
        throw "$Label login expected success=true. Body: $($loginResponse.BodyText)"
    }
    Assert-JsonFieldNotEmpty -Label "$Label login" -Response $loginResponse -Field 'message'

    Write-Step "$Label current user"
    $currentUserResponse = Invoke-CurlJson -Method 'GET' -Url "$BaseUrl/api/auth/user" -CookieJar $CookieJar
    Assert-StatusCode -Label "$Label current user" -Response $currentUserResponse -ExpectedStatusCode 200
    Assert-JsonFieldEquals -Label "$Label current user" -Response $currentUserResponse -Field 'username' -ExpectedValue $Username

    Write-Step "$Label refresh"
    $refreshResponse = Invoke-CurlJson -Method 'POST' -Url "$BaseUrl/api/auth/refresh" -CookieJar $CookieJar
    Assert-StatusCode -Label "$Label refresh" -Response $refreshResponse -ExpectedStatusCode 200

    Write-Step "$Label projects"
    $projectsResponse = Invoke-CurlJson -Method 'GET' -Url "$BaseUrl/api/projects" -CookieJar $CookieJar
    Assert-StatusCode -Label "$Label projects" -Response $projectsResponse -ExpectedStatusCode 200

    Write-Step "$Label logout"
    $logoutResponse = Invoke-CurlJson -Method 'POST' -Url "$BaseUrl/api/auth/logout" -CookieJar $CookieJar
    Assert-StatusCode -Label "$Label logout" -Response $logoutResponse -ExpectedStatusCode 200

    Write-Step "$Label user after logout"
    $userAfterLogoutResponse = Invoke-CurlJson -Method 'GET' -Url "$BaseUrl/api/auth/user" -CookieJar $CookieJar
    Assert-StatusCode -Label "$Label user after logout" -Response $userAfterLogoutResponse -ExpectedStatusCode 401
    Assert-JsonFieldEquals -Label "$Label user after logout" -Response $userAfterLogoutResponse -Field 'detail' -ExpectedValue $notLoggedInDetail
}

Ensure-RequiredTool -CommandName 'curl.exe' | Out-Null

if (-not $PSBoundParameters.ContainsKey('BackendPort')) {
    $BackendPort = [int](Get-EnvValue -Key 'APP_PORT' -Default '8000')
}
if (-not $PSBoundParameters.ContainsKey('DockerAppPort')) {
    $DockerAppPort = [int](Get-EnvValue -Key 'DOCKER_APP_PORT' -Default '8000')
}
if (-not $PSBoundParameters.ContainsKey('Username')) {
    $Username = Get-EnvValue -Key 'LOCAL_AUTH_USERNAME' -Default 'admin'
}
if (-not $PSBoundParameters.ContainsKey('Password')) {
    $Password = Get-EnvValue -Key 'LOCAL_AUTH_PASSWORD' -Default ''
}

if ([string]::IsNullOrWhiteSpace($Password)) {
    throw 'LOCAL_AUTH_PASSWORD is empty. Please set it in .env or pass -Password.'
}

$backendBaseUrl = "http://${BackendHost}:${BackendPort}"
$frontendBaseUrl = "http://${FrontendHost}:${FrontendPort}"
$dockerBaseUrl = "http://${DockerHost}:${DockerAppPort}"
$tmpDir = Join-Path $PSScriptRoot 'tmp'
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null
$loginBodyFile = Join-Path $tmpDir 'auth-smoke-login.json'
$backendCookieJar = Join-Path $tmpDir 'auth-smoke-backend.cookies.txt'
$frontendCookieJar = Join-Path $tmpDir 'auth-smoke-frontend.cookies.txt'

$loginPayload = @{
    username = $Username
    password = $Password
} | ConvertTo-Json -Compress
[System.IO.File]::WriteAllText($loginBodyFile, $loginPayload, $Utf8NoBomEncoding)

Write-Host "Auth smoke check parameters:" -ForegroundColor Yellow
Write-Host "  backend : $backendBaseUrl"
Write-Host "  frontend: $frontendBaseUrl"
Write-Host "  docker  : $dockerBaseUrl"
Write-Host "  username: $Username"

Test-Readyz -Label 'backend' -BaseUrl $backendBaseUrl
Test-AuthFlow -Label 'backend' -BaseUrl $backendBaseUrl -CookieJar $backendCookieJar -LoginBodyFile $loginBodyFile

if (-not $SkipFrontend) {
    Test-AuthFlow -Label 'frontend proxy' -BaseUrl $frontendBaseUrl -CookieJar $frontendCookieJar -LoginBodyFile $loginBodyFile
}

if (-not $SkipDockerReadyz) {
    Test-Readyz -Label 'docker app' -BaseUrl $dockerBaseUrl
}

Write-Host ""
Write-Host 'Auth smoke check passed.' -ForegroundColor Green
