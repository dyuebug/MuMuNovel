[CmdletBinding()]
param(
    [string]$Version,
    [string]$TargetBranch = "dev",
    [string]$Remote = "origin",
    [string]$ChangelogPath = "CHANGELOG.md",
    [string]$ReleaseNotesPath,
    [switch]$SkipPush,
    [switch]$SkipRelease,
    [switch]$AllowDirtyWorktree,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location -Path $PSScriptRoot

$Utf8NoBomEncoding = [System.Text.UTF8Encoding]::new($false)
$LogFilePath = Join-Path $PSScriptRoot "release.log"

function Initialize-LogFile {
    $header = @(
        "=== Release started $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') ===",
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

function Invoke-Git {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,
        [switch]$AllowNonZeroExit
    )

    $escapedArguments = $Arguments | ForEach-Object {
        if ($_ -match '[\s"]') {
            '"' + ($_.Replace('"', '\"')) + '"'
        }
        else {
            $_
        }
    }

    $processInfo = New-Object System.Diagnostics.ProcessStartInfo
    $processInfo.FileName = 'git'
    $processInfo.Arguments = [string]::Join(' ', $escapedArguments)
    $processInfo.RedirectStandardOutput = $true
    $processInfo.RedirectStandardError = $true
    $processInfo.UseShellExecute = $false

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $processInfo
    $null = $process.Start()

    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $process.WaitForExit()

    $exitCode = $process.ExitCode
    $script:LastGitExitCode = $exitCode
    $combinedOutput = @($stdout.TrimEnd(), $stderr.TrimEnd()) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    $textOutput = ($combinedOutput -join [Environment]::NewLine).TrimEnd()

    Write-LogBlock ("git " + ($Arguments -join ' '))
    Write-LogBlock $textOutput

    if (-not $AllowNonZeroExit -and $exitCode -ne 0) {
        throw "git $($Arguments -join ' ') failed with exit code $exitCode."
    }

    if ([string]::IsNullOrWhiteSpace($textOutput)) {
        return @()
    }

    return @($textOutput -split "`r?`n")
}

function Get-FirstGitLine {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,
        [switch]$AllowNonZeroExit
    )

    $result = Invoke-Git -Arguments $Arguments -AllowNonZeroExit:$AllowNonZeroExit
    $text = ($result | Out-String).Trim()
    if ([string]::IsNullOrWhiteSpace($text)) {
        return ''
    }

    return (($text -split "`r?`n")[0]).Trim()
}

function Resolve-ReleaseVersion {
    param([string]$InputVersion)

    if (-not [string]::IsNullOrWhiteSpace($InputVersion)) {
        if ($InputVersion.StartsWith('v')) {
            return $InputVersion
        }

        return "v$InputVersion"
    }

    $packageJsonPath = Join-Path $PSScriptRoot 'frontend/package.json'
    if (-not (Test-Path $packageJsonPath)) {
        throw "Version source not found: $packageJsonPath"
    }

    $packageJson = Get-Content $packageJsonPath -Raw | ConvertFrom-Json
    if ([string]::IsNullOrWhiteSpace($packageJson.version)) {
        throw 'frontend/package.json does not contain a version field.'
    }

    return "v$($packageJson.version.TrimStart('v'))"
}

function Get-CurrentBranchName {
    return (Get-FirstGitLine -Arguments @('branch', '--show-current'))
}

function Assert-CleanWorktree {
    if ($AllowDirtyWorktree) {
        Write-Step 'Skipping worktree cleanliness check (AllowDirtyWorktree).'
        return
    }

    $statusLines = Invoke-Git -Arguments @('status', '--porcelain')
    $hasChanges = $false
    foreach ($line in $statusLines) {
        if (-not [string]::IsNullOrWhiteSpace($line.ToString())) {
            $hasChanges = $true
            break
        }
    }

    if ($hasChanges) {
        throw 'Worktree has uncommitted changes. Commit them first or pass -AllowDirtyWorktree.'
    }
}

function Assert-OnTargetBranch {
    $currentBranch = Get-CurrentBranchName
    if ($currentBranch -ne $TargetBranch) {
        throw "Current branch is $currentBranch, expected $TargetBranch. Switch first before releasing."
    }
}

function Assert-ChangelogEntry {
    param([string]$ResolvedVersion)

    $changelogFullPath = Join-Path $PSScriptRoot $ChangelogPath
    if (-not (Test-Path $changelogFullPath)) {
        throw "Changelog file not found: $changelogFullPath"
    }

    $changelogContent = Get-Content $changelogFullPath -Raw
    $escapedVersion = [Regex]::Escape($ResolvedVersion)
    if ($changelogContent -notmatch "(?m)^##\s+$escapedVersion(\s+-.*)?$") {
        throw "CHANGELOG entry not found for $ResolvedVersion"
    }
}

function Get-ReleaseNotesBody {
    param([string]$ResolvedVersion)

    $effectiveReleaseNotesPath = $ReleaseNotesPath
    if ([string]::IsNullOrWhiteSpace($effectiveReleaseNotesPath)) {
        $effectiveReleaseNotesPath = Join-Path 'docs/releases' "$ResolvedVersion.md"
    }

    $releaseNotesFullPath = Join-Path $PSScriptRoot $effectiveReleaseNotesPath
    if (Test-Path $releaseNotesFullPath) {
        Write-Step "Using release notes file: $effectiveReleaseNotesPath"
        $content = Get-Content $releaseNotesFullPath -Raw
        if ([string]::IsNullOrWhiteSpace($content)) {
            throw "Release notes file is empty: $effectiveReleaseNotesPath"
        }

        return $content.Trim()
    }

    Write-Step 'Release notes file not found. Falling back to CHANGELOG section.'
    $changelogFullPath = Join-Path $PSScriptRoot $ChangelogPath
    $changelogContent = Get-Content $changelogFullPath -Raw
    $escapedVersion = [Regex]::Escape($ResolvedVersion)
    $pattern = "(?ms)^##\s+$escapedVersion(?:\s+-.*)?\r?\n.*?(?=^##\s+|\z)"
    $match = [Regex]::Match($changelogContent, $pattern)
    if (-not $match.Success) {
        throw "Unable to extract release notes for $ResolvedVersion from CHANGELOG."
    }

    $body = $match.Value.Trim()
    if ([string]::IsNullOrWhiteSpace($body)) {
        throw 'Extracted release notes body is empty.'
    }

    return $body
}

function Get-HeadCommit {
    return (Get-FirstGitLine -Arguments @('rev-parse', 'HEAD'))
}

function Get-LocalTagCommit {
    param([string]$TagName)

    Invoke-Git -Arguments @('rev-parse', '-q', '--verify', "refs/tags/$TagName") -AllowNonZeroExit | Out-Null
    if ($script:LastGitExitCode -ne 0) {
        return $null
    }

    return (Get-FirstGitLine -Arguments @('rev-list', '-n', '1', $TagName))
}

function Get-RemoteTagCommit {
    param(
        [string]$RemoteName,
        [string]$TagName
    )

    $lines = Invoke-Git -Arguments @('ls-remote', '--tags', $RemoteName, "refs/tags/$TagName", "refs/tags/$TagName^{}") -AllowNonZeroExit
    if ($script:LastGitExitCode -ne 0) {
        throw "Unable to read remote tag $TagName from $RemoteName"
    }

    $resolvedCommit = $null
    $fallbackCommit = $null
    foreach ($line in $lines) {
        $text = $line.ToString().Trim()
        if ([string]::IsNullOrWhiteSpace($text)) {
            continue
        }

        $parts = $text -split "`t"
        if ($parts.Count -lt 2) {
            continue
        }

        if ($parts[1].EndsWith('^{}')) {
            $resolvedCommit = $parts[0]
        }
        else {
            $fallbackCommit = $parts[0]
        }
    }

    if ($resolvedCommit) {
        return $resolvedCommit
    }

    return $fallbackCommit
}

function Ensure-LocalTag {
    param(
        [string]$TagName,
        [string]$ExpectedCommit
    )

    $existingCommit = Get-LocalTagCommit -TagName $TagName
    if ($existingCommit) {
        if ($existingCommit -ne $ExpectedCommit) {
            throw "Local tag $TagName already exists on $existingCommit, not on $ExpectedCommit."
        }

        Write-Step "Local tag already exists: $TagName -> $existingCommit"
        return
    }

    if ($DryRun) {
        Write-Step "DryRun: would create local annotated tag $TagName"
        return
    }

    Write-Step "Creating local tag: $TagName"
    Invoke-Git -Arguments @('tag', '-a', $TagName, '-m', "Release $TagName") | Out-Null
}

function Push-ReleaseRefs {
    param(
        [string]$TagName,
        [string]$ExpectedCommit
    )

    $remoteTagCommit = Get-RemoteTagCommit -RemoteName $Remote -TagName $TagName
    if ($remoteTagCommit -and $remoteTagCommit -ne $ExpectedCommit) {
        throw "Remote tag $TagName already exists on $remoteTagCommit, not on $ExpectedCommit."
    }

    if ($SkipPush) {
        Write-Step 'Skipping push step (SkipPush).'
        return
    }

    if ($DryRun) {
        Write-Step "DryRun: would push branch $TargetBranch and tag $TagName to $Remote"
        return
    }

    Write-Step "Pushing branch to $Remote/$TargetBranch"
    Invoke-Git -Arguments @('push', $Remote, $TargetBranch) | Out-Null

    if ($remoteTagCommit) {
        Write-Step "Remote tag already exists: $TagName -> $remoteTagCommit"
        return
    }

    Write-Step "Pushing tag to ${Remote}: $TagName"
    Invoke-Git -Arguments @('push', $Remote, $TagName) | Out-Null
}

function Get-GitHubRepoInfo {
    $remoteUrl = Get-FirstGitLine -Arguments @('remote', 'get-url', $Remote)

    if ($remoteUrl -match '^https://github\.com/(?<owner>[^/]+)/(?<repo>[^/.]+?)(?:\.git)?$') {
        return @{
            owner = $Matches.owner
            repo = $Matches.repo
        }
    }

    if ($remoteUrl -match '^git@github\.com:(?<owner>[^/]+)/(?<repo>[^/.]+?)(?:\.git)?$') {
        return @{
            owner = $Matches.owner
            repo = $Matches.repo
        }
    }

    throw "Only GitHub remotes are supported. Could not parse: $remoteUrl"
}

function Get-GitHubCredential {
    $processInfo = New-Object System.Diagnostics.ProcessStartInfo
    $processInfo.FileName = 'git'
    $processInfo.Arguments = 'credential-manager get --no-ui'
    $processInfo.RedirectStandardInput = $true
    $processInfo.RedirectStandardOutput = $true
    $processInfo.RedirectStandardError = $true
    $processInfo.UseShellExecute = $false

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $processInfo
    $null = $process.Start()
    $process.StandardInput.Write("protocol=https`nhost=github.com`n`n")
    $process.StandardInput.Close()

    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $process.WaitForExit()

    if ($process.ExitCode -ne 0) {
        throw "Git Credential Manager failed to return a GitHub credential: $stderr"
    }

    $result = @{}
    foreach ($line in ($stdout -split "`r?`n")) {
        if ($line -match '^(.*?)=(.*)$') {
            $result[$Matches[1]] = $Matches[2]
        }
    }

    if (-not $result.ContainsKey('username') -or -not $result.ContainsKey('password')) {
        throw 'Git Credential Manager did not return username/password.'
    }

    return $result
}

function Invoke-GitHubApi {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('GET', 'POST', 'PATCH')]
        [string]$Method,
        [Parameter(Mandatory = $true)]
        [string]$Uri,
        [object]$Body,
        [switch]$TreatNotFoundAsNull
    )

    $credential = Get-GitHubCredential
    $basicToken = [Convert]::ToBase64String(
        [System.Text.Encoding]::ASCII.GetBytes("$($credential.username):$($credential.password)")
    )

    $headers = @{
        Authorization = "Basic $basicToken"
        Accept = 'application/vnd.github+json'
        'X-GitHub-Api-Version' = '2022-11-28'
        'User-Agent' = 'MuMuNovel-release-script'
    }

    try {
        Write-LogBlock ("GitHub API $Method $Uri")
        if ($null -ne $Body) {
            $json = $Body | ConvertTo-Json -Depth 10 -Compress
            return Invoke-RestMethod -Method $Method -Uri $Uri -Headers $headers -Body ([System.Text.Encoding]::UTF8.GetBytes($json)) -ContentType 'application/json; charset=utf-8'
        }

        return Invoke-RestMethod -Method $Method -Uri $Uri -Headers $headers
    }
    catch {
        $statusCode = $null
        if ($_.Exception.Response) {
            $statusCode = [int]$_.Exception.Response.StatusCode
        }

        if ($TreatNotFoundAsNull -and $statusCode -eq 404) {
            return $null
        }

        throw
    }
}

function Sync-GitHubRelease {
    param(
        [string]$ResolvedVersion,
        [string]$ReleaseBody
    )

    if ($SkipRelease) {
        Write-Step 'Skipping GitHub Release step (SkipRelease).'
        return
    }

    $repoInfo = Get-GitHubRepoInfo
    $baseUri = "https://api.github.com/repos/$($repoInfo.owner)/$($repoInfo.repo)"
    $existingRelease = Invoke-GitHubApi -Method GET -Uri "$baseUri/releases/tags/$ResolvedVersion" -TreatNotFoundAsNull

    if ($existingRelease) {
        if ($DryRun) {
            Write-Step "DryRun: would update GitHub Release $ResolvedVersion"
            return
        }

        Write-Step "Updating GitHub Release: $ResolvedVersion"
        $patchBody = @{
            tag_name = $ResolvedVersion
            target_commitish = $TargetBranch
            name = $ResolvedVersion
            body = $ReleaseBody
            draft = $false
            prerelease = $false
        }
        $updatedRelease = Invoke-GitHubApi -Method PATCH -Uri "$baseUri/releases/$($existingRelease.id)" -Body $patchBody
        Write-Host "Release URL: $($updatedRelease.html_url)"
        Write-LogLine "Release URL: $($updatedRelease.html_url)"
        return
    }

    if ($DryRun) {
        Write-Step "DryRun: would create GitHub Release $ResolvedVersion"
        return
    }

    Write-Step "Creating GitHub Release: $ResolvedVersion"
    $postBody = @{
        tag_name = $ResolvedVersion
        target_commitish = $TargetBranch
        name = $ResolvedVersion
        body = $ReleaseBody
        draft = $false
        prerelease = $false
    }
    $createdRelease = Invoke-GitHubApi -Method POST -Uri "$baseUri/releases" -Body $postBody
    Write-Host "Release URL: $($createdRelease.html_url)"
    Write-LogLine "Release URL: $($createdRelease.html_url)"
}

Initialize-LogFile

$resolvedVersion = Resolve-ReleaseVersion -InputVersion $Version
$resolvedNotesPath = $ReleaseNotesPath
if ([string]::IsNullOrWhiteSpace($resolvedNotesPath)) {
    $resolvedNotesPath = Join-Path 'docs/releases' "$resolvedVersion.md"
}

Write-Step "Preparing release $resolvedVersion"
Write-Host "Target branch: $TargetBranch"
Write-Host "Remote: $Remote"
Write-Host "Release notes: $resolvedNotesPath"
Write-Host "Log file: $LogFilePath"
Write-LogLine "Target branch: $TargetBranch"
Write-LogLine "Remote: $Remote"
Write-LogLine "Release notes: $resolvedNotesPath"

Assert-OnTargetBranch
Assert-CleanWorktree
Assert-ChangelogEntry -ResolvedVersion $resolvedVersion
$releaseBody = Get-ReleaseNotesBody -ResolvedVersion $resolvedVersion
$headCommit = Get-HeadCommit

Write-Step "Current commit: $headCommit"
Ensure-LocalTag -TagName $resolvedVersion -ExpectedCommit $headCommit
Push-ReleaseRefs -TagName $resolvedVersion -ExpectedCommit $headCommit
Sync-GitHubRelease -ResolvedVersion $resolvedVersion -ReleaseBody $releaseBody

Write-Step 'Release flow completed'
Write-Host "Version: $resolvedVersion"
Write-Host "Commit: $headCommit"
Write-Host 'Done.'
Write-LogLine "Version: $resolvedVersion"
Write-LogLine "Commit: $headCommit"
Write-LogLine 'Release flow completed successfully.'

