<#
.SYNOPSIS
    Read-only verification of an installed RTX Spark AI Control.

.DESCRIPTION
    Answers one question without changing the answer: is what is on this machine
    a valid installation, correctly configured, with nothing extra bolted on?

    Read-only means exactly that. This script installs nothing, downloads
    nothing, writes no setting, starts no server and asks no model to load. It
    reads a hash, a registry key, a shortcut, a settings file, a process list and
    (unless -SkipNetwork) one TCP connect.

    It also checks the things this application promises *not* to do: no service,
    no scheduled task and no Run entry, and an inference server that this check
    neither started nor stopped. On battery it says so rather than failing: the
    battery gate is designed behaviour.

.PARAMETER AppPath
    Verify this executable instead of the per-user package location. Use it for a
    bare build you run from a folder.

.PARAMETER SettingsPath
    Verify this settings file instead of
    %APPDATA%\com.rtxspark.aicontrol.v2\settings.json. V2_TEST_DATA_DIR is
    honoured by the application and by this script.

.PARAMETER InstallerPath
    Hash this downloaded installer and compare it with the SHA-256 published for
    it. A mismatch is a failure: do not run that file.

.PARAMETER ExpectedInstallerSha256
    Compare against this value instead of the one in release/v0.1.0/SHA256SUMS.txt.

.PARAMETER SkipNetwork
    Do not probe for network reachability.

.PARAMETER Json
    Emit one JSON document instead of the aligned report.

.EXAMPLE
    powershell -File tools\verify-install.ps1

.EXAMPLE
    powershell -File tools\verify-install.ps1 -InstallerPath "$env:USERPROFILE\Downloads\RTX-Spark-AI-Control_0.1.0_arm64-setup.exe"

.NOTES
    Exit codes: 0 verified, 1 a required check failed, 2 unsupported platform,
    3 not installed, 4 the checks themselves could not run.
#>
param(
    [string]$AppPath = '',
    [string]$SettingsPath = '',
    [string]$InstallerPath = '',
    [string]$ExpectedInstallerSha256 = '',
    [switch]$SkipNetwork,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'lib\install-checks.ps1')

# What Setup requires before it will save, and what kind each path field is. The
# app validates the same way; this mirrors its rules so a report is comparable.
$setupFields = @('vllm_root', 'vllm_exe', 'model_checkpoint', 'launch_config', 'chat_template')
$pathKinds = [ordered]@{
    vllm_root        = 'folder'
    vllm_exe         = 'file'
    model_checkpoint = 'folder'
    launch_config    = 'file'
    chat_template    = 'file'
    python_exe       = 'file'
    blackout_engine  = 'file'
    blackout_python  = 'file'
    logs_dir         = 'folder'
    legacy_config    = 'file'
}

# The published hashes for 0.1.0, from release/v0.1.0/RELEASE-MANIFEST.md section 2.
# The installed copy differs from the pre-bundle build by three bytes, where Tauri
# rewrites its bundle-type marker, so both are legitimate artifacts of a release.
$knownHashes = @{
    '0.1.0' = @(
        'dcba0ea3804bcb1adf25fc188b564e5b98311dc4a6e21a88dbe56773618d63b0',
        'ee57faabd6047a4e38e13e0e6f2cac7ed0e3f5a1d28721b1d6f1340ddae43990'
    )
}

$notes = @()
try {
    $facts = @{}
    $facts += Get-PlatformFacts

    $installDir = Join-Path $env:LOCALAPPDATA 'RTX Spark AI Control'
    $appExe = if ($AppPath) { $AppPath } else { Join-Path $installDir 'rtx-spark-ai-control-v2.exe' }
    $facts.AppExe = $appExe
    $facts.AppExePresent = [bool](Test-Path -LiteralPath $appExe)

    $actualSha = ''
    if ($facts.AppExePresent) {
        try {
            $actualSha = (Get-FileHash -LiteralPath $appExe -Algorithm SHA256 -ErrorAction Stop).Hash
            $facts.AppExeSha256 = $actualSha
        } catch {
            $notes += ('the executable could not be hashed: ' + $_.Exception.Message)
        }

        $versionText = ''
        try {
            $info = (Get-Item -LiteralPath $appExe -ErrorAction Stop).VersionInfo
            $versionText = [string]$info.ProductVersion
            if (-not $versionText) { $versionText = [string]$info.FileVersion }
        } catch {
            $versionText = ''
        }
        $version = ''
        if ($versionText -match '\d+\.\d+\.\d+') { $version = $Matches[0] }
        if ($version) {
            $facts.AppVersion = $version
            if ($knownHashes.ContainsKey($version)) { $facts.KnownInstalledSha256 = $knownHashes[$version] }
        } else {
            $notes += 'the executable reported no version, so no published hash was compared'
        }
    }

    $facts.UninstallKeyPresent = [bool](Test-Path -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RTX Spark AI Control')
    $shortcut = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\RTX Spark AI Control.lnk'
    $facts.ShortcutPresent = [bool](Test-Path -LiteralPath $shortcut)

    try {
        $facts.ServicePresent = @(Get-Service -Name 'RTX*' -ErrorAction SilentlyContinue |
                Where-Object { $_.Name -like '*rtx-spark-ai-control*' -or $_.DisplayName -like '*RTX Spark AI Control*' }).Count -gt 0
    } catch {
        $facts.ServicePresent = $false
    }
    try {
        $facts.ScheduledTaskPresent = @(Get-ScheduledTask -TaskName 'RTX*' -ErrorAction SilentlyContinue |
                Where-Object { $_.TaskName -like '*RTX Spark AI Control*' -or $_.TaskName -like '*rtx-spark-ai-control*' }).Count -gt 0
        # Vendor components use similar names: an NVIDIA "RTXSparkFirstRunNotification"
        # task and an "RTX" service family both exist on supported hardware and belong
        # to neither this application nor its installer. Ignoring them is the point,
        # but saying so avoids reading the pass below as a missed finding.
        $unrelated = @(Get-ScheduledTask -TaskName 'RTX*' -ErrorAction SilentlyContinue |
                Where-Object { $_.TaskName -notlike '*RTX Spark AI Control*' -and $_.TaskName -notlike '*rtx-spark-ai-control*' })
        if ($unrelated.Count -gt 0) {
            $notes += ("{0} unrelated scheduled task(s) whose name starts with RTX were ignored (vendor components, not this application)" -f $unrelated.Count)
        }
    } catch {
        $facts.ScheduledTaskPresent = $false
    }
    $runKeyPresent = $false
    try {
        $runKey = Get-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -ErrorAction Stop
        foreach ($property in $runKey.PSObject.Properties) {
            if ($property.Name -like 'PS*') { continue }
            if ($property.Name -like '*RTX*' -or [string]$property.Value -like '*rtx-spark*') { $runKeyPresent = $true }
        }
    } catch {
        $runKeyPresent = $false
    }
    $facts.RunKeyPresent = $runKeyPresent
} catch {
    Write-Output ('the checks could not run: ' + $_.Exception.Message)
    exit 4
}

try {
    if ($SettingsPath) {
        $settingsFile = $SettingsPath
    } elseif ($env:V2_TEST_DATA_DIR) {
        $settingsFile = Join-Path $env:V2_TEST_DATA_DIR 'settings.json'
    } else {
        $settingsFile = Join-Path $env:APPDATA 'com.rtxspark.aicontrol.v2\settings.json'
    }
    $facts.SettingsPath = $settingsFile
    $facts.SettingsExists = [bool](Test-Path -LiteralPath $settingsFile)

    $missingFields = @()
    $invalidPaths = @()
    if ($facts.SettingsExists) {
        try {
            $settings = Get-Content -LiteralPath $settingsFile -Raw -ErrorAction Stop | ConvertFrom-Json
            $facts.SettingsSchema = [int]$settings.schema
            $facts.AutostartInference = [bool]$settings.autostart_inference

            $paths = $settings.paths
            if ($null -eq $paths) {
                foreach ($field in $setupFields) { $missingFields += $field }
                $notes += 'the settings file has no paths object'
            } else {
                foreach ($field in $setupFields) {
                    $value = ''
                    if ($paths.PSObject.Properties.Name -contains $field) { $value = [string]$paths.$field }
                    if ([string]::IsNullOrWhiteSpace($value)) { $missingFields += $field }
                }
                foreach ($entry in $pathKinds.GetEnumerator()) {
                    if (-not ($paths.PSObject.Properties.Name -contains $entry.Key)) { continue }
                    $value = [string]$paths.($entry.Key)
                    if ([string]::IsNullOrWhiteSpace($value)) { continue }
                    $exists = Test-Path -LiteralPath $value
                    if (-not $exists) {
                        $invalidPaths += @{ Field = $entry.Key; Value = $value; Reason = 'does not exist' }
                        continue
                    }
                    $isFolder = Test-Path -LiteralPath $value -PathType Container
                    if ($entry.Value -eq 'folder' -and -not $isFolder) {
                        $invalidPaths += @{ Field = $entry.Key; Value = $value; Reason = 'is a file, expected a folder' }
                    }
                    if ($entry.Value -eq 'file' -and $isFolder) {
                        $invalidPaths += @{ Field = $entry.Key; Value = $value; Reason = 'is a folder, expected a file' }
                    }
                }
                if ($paths.vllm_root) {
                    $root = [string]$paths.vllm_root
                    if ($root -and -not (Test-Path -LiteralPath (Join-Path $root 'build-info.json'))) {
                        $notes += ('{0} has no build-info.json, so the Updates page cannot report a version' -f $root)
                    }
                }
                $facts.BlackoutEngine = [string]$paths.blackout_engine
                $facts.BlackoutPython = [string]$paths.blackout_python
                $facts.ApiHost = [string]$paths.api_host
                if ($paths.api_port) { $facts.ApiPort = [int]$paths.api_port }
            }
        } catch {
            $facts.ConfigurationError = $_.Exception.Message
        }
    }
    $facts.MissingFields = $missingFields
    $facts.InvalidPaths = $invalidPaths

    $facts.VllmProcessCount = Get-ProcessCount -Name 'vllm'
    $facts.PythonProcessCount = Get-ProcessCount -Name @('python', 'pythonw')

    $startupFile = Join-Path $env:TEMP 'rtx-v2-startup.json'
    if (Test-Path -LiteralPath $startupFile) {
        try {
            $facts.StartupReport = Get-Content -LiteralPath $startupFile -Raw -ErrorAction Stop | ConvertFrom-Json
        } catch {
            $notes += 'the startup report could not be read'
        }
    }

    if ($InstallerPath) {
        $facts.InstallerPath = $InstallerPath
        if (Test-Path -LiteralPath $InstallerPath) {
            try {
                $facts.InstallerSha256 = (Get-FileHash -LiteralPath $InstallerPath -Algorithm SHA256 -ErrorAction Stop).Hash
            } catch {
                $notes += ('the installer could not be hashed: ' + $_.Exception.Message)
            }
        } else {
            $notes += ('the installer path does not exist: ' + $InstallerPath)
        }

        $expected = $ExpectedInstallerSha256
        if (-not $expected -and $facts.InstallerSha256) {
            $sums = Join-Path $PSScriptRoot '..\release\v0.1.0\SHA256SUMS.txt'
            if (Test-Path -LiteralPath $sums) {
                $name = Split-Path -Leaf $InstallerPath
                foreach ($line in Get-Content -LiteralPath $sums -ErrorAction SilentlyContinue) {
                    if ($line -match '^([0-9a-fA-F]{64})\s+\*?(.+)$') {
                        if ($Matches[2].Trim() -eq $name) { $expected = $Matches[1] }
                    }
                }
                if (-not $expected) {
                    $notes += ('no published hash for {0} in release/v0.1.0/SHA256SUMS.txt' -f $name)
                }
            }
        }
        $facts.ExpectedInstallerSha256 = $expected
    }

    $facts.PowerOnline = Get-PowerSource
    if (-not $SkipNetwork) { $facts.NetworkReachable = Test-EndpointReachable }
} catch {
    Write-Output ('the checks could not run: ' + $_.Exception.Message)
    exit 4
}

$verdicts = @(Get-InstallVerdicts -Facts $facts)
$exitCode = Get-CheckExitCode -Verdicts $verdicts -NotInstalledIsDistinct

if ($Json) {
    $payload = [ordered]@{
        script   = 'verify-install'
        machine  = [ordered]@{
            build        = [int](Get-Fact $facts 'OSBuild' 0)
            architecture = [string](Get-Fact $facts 'Architecture' 'unknown')
            powerOnline  = Get-Fact $facts 'PowerOnline' $null
        }
        install  = [ordered]@{
            appExe     = [string](Get-Fact $facts 'AppExe' '')
            version    = [string](Get-Fact $facts 'AppVersion' '')
            settings   = [string](Get-Fact $facts 'SettingsPath' '')
            installer  = [string](Get-Fact $facts 'InstallerPath' '')
        }
        exitCode = $exitCode
        notes    = @($notes | Where-Object { $_ })
        verdicts = @($verdicts | ForEach-Object {
                [ordered]@{ id = $_.Id; label = $_.Label; kind = $_.Kind; status = $_.Status; detail = $_.Detail; remedy = $_.Remedy }
            })
    }
    $payload | ConvertTo-Json -Depth 6
    exit $exitCode
}

Write-Output 'RTX Spark AI Control - install verification (read-only)'
Write-Output 'Nothing below installed, downloaded, started or changed anything.'
Write-VerdictReport -Verdicts $verdicts -Title 'Checks'

foreach ($note in @($notes | Where-Object { $_ })) {
    Write-Output ('note: ' + $note)
}

Write-VerdictSummary -Verdicts $verdicts

$message = switch ($exitCode) {
    0 { 'Verified: what is installed matches what the documentation describes.' }
    1 { 'Verification failed: at least one required check did not hold. Each failure above says why.' }
    2 { 'Unsupported platform: RTX Spark AI Control v0.1.0 targets Windows 11 ARM64 only.' }
    3 { 'Not installed: no application was found. Use AGENT-INSTALL.md or INSTALL.md when you decide to install.' }
    default { 'The checks could not run.' }
}
Write-Output ''
Write-Output $message
exit $exitCode
