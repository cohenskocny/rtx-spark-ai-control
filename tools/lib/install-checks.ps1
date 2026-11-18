<#
.SYNOPSIS
    Shared checks and probes for the read-only installer tooling.

.DESCRIPTION
    `tools/check-requirements.ps1` and `tools/verify-install.ps1` make no
    judgements of their own: they gather facts about the machine, hand them to
    the pure functions here, and print the verdicts. Keeping the decisions pure
    is what makes the awkward cases testable without a second machine, a real
    installer or a GPU - `tools/test-install-scripts.ps1` feeds them fabricated
    fact sets for x64 Windows, a missing driver, a missing WebView2 runtime, an
    absent blackout engine, a battery-only laptop, a checksum mismatch, an
    offline host and a user who declines to install.

    Pure functions: ConvertFrom-ArchitectureCode, Test-Sha256Match,
    Get-PlatformVerdicts, Get-RequirementVerdicts, Get-ConfigurationVerdicts,
    Get-InstallVerdicts, Get-VerdictCounts, Test-UnsupportedPlatform,
    Get-CheckExitCode, Write-VerdictReport, Write-VerdictSummary.
    They read no file, start no process and open no socket.

    Probe functions: Get-PlatformFacts, Get-GpuFacts, Get-WebView2Version,
    Get-FreeSpaceFacts, Read-BuildInfo, Find-VllmInstallation,
    Find-VllmLauncher, Find-BlackoutScript, Get-PowerSource,
    Test-EndpointReachable, Get-ProcessCount.
    They only read: a CIM query, a registry value, a file's text, a directory
    listing, a process list or a TCP connect. Nothing in this file installs,
    downloads, starts a server, loads a model, writes a setting or changes a
    driver.

    A verdict is a small object:

        Id      stable identifier, used by the tests and by the JSON output
        Label   what is being checked, in the operator's words
        Kind    required | optional | informational
        Status  pass | warn | fail | skip
        Detail  what was found, including the number that was found
        Remedy  what the operator can do next, or empty when nothing is needed

    `skip` means "does not apply here" - an absent optional component, an
    unconfigured feature - never "we could not tell". A check that cannot run
    reports `warn` and says why, because a silent gap is the one thing a
    verification script must not produce.
#>

# The Windows build number of the first Windows 11 release. Below it is Windows
# 10, which is not a target.
$script:Windows11Build = 22000

# ----------------------------------------------------------------------- pure

function Get-Fact {
    <#
    .SYNOPSIS
        Read one fact with a default, so a partial fact set stays a valid input.

    .DESCRIPTION
        The tests build fact sets by hand; a missing key must mean "not probed"
        rather than throw. Every read in this file goes through here.
    #>
    param(
        $Facts,
        [Parameter(Mandatory = $true)][string]$Key,
        $Default = $null
    )
    if ($null -ne $Facts -and $Facts.ContainsKey($Key)) {
        $value = $Facts[$Key]
        if ($null -ne $value) { return $value }
    }
    return $Default
}

function New-Verdict {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [Parameter(Mandatory = $true)][string]$Label,
        [ValidateSet('required', 'optional', 'informational')][string]$Kind = 'required',
        [ValidateSet('pass', 'warn', 'fail', 'skip')][string]$Status = 'pass',
        [string]$Detail = '',
        [string]$Remedy = ''
    )
    [pscustomobject]@{
        Id     = $Id
        Label  = $Label
        Kind   = $Kind
        Status = $Status
        Detail = $Detail
        Remedy = $Remedy
    }
}

function ConvertFrom-ArchitectureCode {
    <#
    .SYNOPSIS
        Turn a `Win32_Processor.Architecture` code into a name.

    .DESCRIPTION
        The environment variables are not trustworthy on an ARM64 machine: a
        64-bit x86 process under emulation reports PROCESSOR_ARCHITECTURE as
        AMD64, which is exactly what Windows PowerShell 5.1 does on an RTX Spark
        laptop. The CIM code is the operating system's own answer, so it wins.
        12 is ARM64, 9 is x64, 0 is x86, 5 is ARM32; anything else is unknown
        rather than guessed.
    #>
    param(
        [int]$Code = -1,
        [string]$SystemType = ''
    )
    switch ($Code) {
        12 { return 'ARM64' }
        9 { return 'AMD64' }
        0 { return 'x86' }
        5 { return 'ARM32' }
    }
    if ($SystemType -like 'ARM64*') { return 'ARM64' }
    return 'unknown'
}

function Test-Sha256Match {
    <#
    .SYNOPSIS
        Compare two SHA-256 values, tolerating case and stray whitespace.

    .DESCRIPTION
        `Comparable = $false` when either side is missing: an absent hash is not
        a mismatch, and reporting it as one would fail a sound installation over
        a value nobody published.
    #>
    param([string]$Expected = '', [string]$Actual = '')
    $e = ($Expected -replace '\s', '').ToLowerInvariant()
    $a = ($Actual -replace '\s', '').ToLowerInvariant()
    if ([string]::IsNullOrWhiteSpace($e) -or [string]::IsNullOrWhiteSpace($a)) {
        return [pscustomobject]@{ Comparable = $false; Ok = $false; Expected = $e; Actual = $a }
    }
    [pscustomobject]@{ Comparable = $true; Ok = ($e -eq $a); Expected = $e; Actual = $a }
}

function Get-PlatformVerdicts {
    <#
    .SYNOPSIS
        Windows 11 ARM64 is the only supported platform; state it with numbers.
    #>
    param($Facts)

    $build = [int](Get-Fact $Facts 'OSBuild' 0)
    $caption = [string](Get-Fact $Facts 'OSCaption' '')
    $architecture = [string](Get-Fact $Facts 'Architecture' 'unknown')
    $source = [string](Get-Fact $Facts 'ArchitectureSource' '')
    $captionSuffix = ''
    if ($caption) { $captionSuffix = " ($caption)" }
    $sourceSuffix = ''
    if ($source) { $sourceSuffix = " ($source)" }

    if ($build -ge $script:Windows11Build) {
        $osVerdict = New-Verdict -Id 'platform_os' -Label 'Windows version' `
            -Detail ("Windows 11, build {0}{1}" -f $build, $captionSuffix)
    } else {
        $osVerdict = New-Verdict -Id 'platform_os' -Label 'Windows version' -Status 'fail' `
            -Detail ("build {0}{1}" -f $build, $captionSuffix) `
            -Remedy 'Windows 11 build 22000 or newer is required. This check upgrades nothing.'
    }

    if ($architecture -eq 'ARM64') {
        $archVerdict = New-Verdict -Id 'platform_arch' -Label 'Processor architecture' -Detail ("ARM64{0}" -f $sourceSuffix)
    } else {
        $archVerdict = New-Verdict -Id 'platform_arch' -Label 'Processor architecture' -Status 'fail' `
            -Detail ("{0}{1}; ARM64 is required" -f $architecture, $sourceSuffix) `
            -Remedy 'v0.1.0 ships an ARM64-only package that refuses to install here. No bypass is offered and no x64 build exists.'
    }

    @($osVerdict, $archVerdict)
}

function Get-RequirementVerdicts {
    <#
    .SYNOPSIS
        What a machine needs before RTX Spark AI Control can run.

    .PARAMETER RequireInference
        Treat a missing native-vLLM installation as a failure rather than a note.
        The GUI runs without one; a server cannot start without one.

    .PARAMETER MinimumFreeGB
        Free space the installation drive should have. The application is under
        6 MB; this figure is working room, not model storage.
    #>
    param(
        $Facts,
        [double]$MinimumFreeGB = 20,
        [switch]$RequireInference
    )

    $verdicts = @()
    $verdicts += Get-PlatformVerdicts -Facts $Facts

    $gpuName = [string](Get-Fact $Facts 'GpuName' '')
    $driver = [string](Get-Fact $Facts 'DriverVersion' '')
    $memoryMiB = [int](Get-Fact $Facts 'GpuMemoryMiB' 0)
    if ($gpuName) {
        $detail = $gpuName
        if ($memoryMiB -gt 0) { $detail += (" | {0:N0} MiB" -f $memoryMiB) }
        $verdicts += New-Verdict -Id 'gpu' -Label 'NVIDIA GPU' -Detail $detail
    } else {
        $verdicts += New-Verdict -Id 'gpu' -Label 'NVIDIA GPU' -Status 'fail' `
            -Detail 'nvidia-smi did not name a GPU' `
            -Remedy 'Install the vendor driver for this hardware yourself. This check installs and changes no driver.'
    }

    if ($driver) {
        $verdicts += New-Verdict -Id 'driver' -Label 'NVIDIA driver' -Kind 'informational' `
            -Detail ("{0} (this release was tested on 621.14)" -f $driver)
    } else {
        $verdicts += New-Verdict -Id 'driver' -Label 'NVIDIA driver' -Kind 'informational' -Status 'warn' `
            -Detail 'no driver version was reported' `
            -Remedy 'GPU telemetry and the inference server both need a working driver. Nothing here can install one.'
    }

    $webview2 = [string](Get-Fact $Facts 'WebView2Version' '')
    if ($webview2) {
        $verdicts += New-Verdict -Id 'webview2' -Label 'WebView2 runtime' -Detail ("{0} installed" -f $webview2)
    } else {
        $verdicts += New-Verdict -Id 'webview2' -Label 'WebView2 runtime' -Status 'fail' `
            -Detail 'no WebView2 runtime found' `
            -Remedy 'The window cannot render without it. Windows 11 normally ships it; repair the Evergreen runtime yourself if it is missing.'
    }

    $freeGB = [double](Get-Fact $Facts 'FreeDiskGB' 0)
    $drive = [string](Get-Fact $Facts 'DiskDrive' 'C:')
    if ($freeGB -ge $MinimumFreeGB) {
        $verdicts += New-Verdict -Id 'disk' -Label 'Free space' -Detail ("{0:N1} GB free on {1}" -f $freeGB, $drive)
    } else {
        $verdicts += New-Verdict -Id 'disk' -Label 'Free space' -Status 'fail' `
            -Detail ("{0:N1} GB free on {1}; {2:N0} GB is the minimum" -f $freeGB, $drive, $MinimumFreeGB) `
            -Remedy 'Free space on the installation drive. A model checkpoint lives elsewhere and is much larger.'
    }

    $pythonPath = [string](Get-Fact $Facts 'PythonPath' '')
    if ($pythonPath) {
        $verdicts += New-Verdict -Id 'python' -Label 'Python interpreter' -Kind 'optional' -Detail $pythonPath
    } else {
        $verdicts += New-Verdict -Id 'python' -Label 'Python interpreter' -Kind 'optional' -Status 'skip' `
            -Detail 'not found on PATH; not required unless the optional blackout guard is configured'
    }

    $vllmRoot = [string](Get-Fact $Facts 'VllmRoot' '')
    $vllmExe = [string](Get-Fact $Facts 'VllmExe' '')
    $vllmVersion = [string](Get-Fact $Facts 'VllmVersion' '')
    $vllmKind = 'optional'
    if ($RequireInference) { $vllmKind = 'required' }
    if ($vllmRoot -and $vllmExe) {
        $versionLabel = $vllmVersion
        if (-not $versionLabel) { $versionLabel = '(version not reported)' }
        $verdicts += New-Verdict -Id 'vllm' -Label 'native-vLLM installation' -Kind $vllmKind `
            -Detail ("native-vLLM {0} at {1}" -f $versionLabel, $vllmRoot)
    } elseif ($vllmRoot -or $vllmExe) {
        # Half an installation means the operator pointed at one and one half is
        # wrong. That is a required failure whatever -RequireInference says: an
        # absent engine is a choice, a broken one is not.
        $verdicts += New-Verdict -Id 'vllm' -Label 'native-vLLM installation' -Kind 'required' -Status 'fail' `
            -Detail ("incomplete: root '{0}', launcher '{1}'" -f $vllmRoot, $vllmExe) `
            -Remedy 'Both halves are needed: the folder holding build-info.json and the vllm.exe inside its environment.'
    } else {
        $status = 'warn'
        if ($RequireInference) { $status = 'fail' }
        $verdicts += New-Verdict -Id 'vllm' -Label 'native-vLLM installation' -Kind $vllmKind -Status $status `
            -Detail 'no installation found' `
            -Remedy 'Install it from https://github.com/devjeonghwan/vllm-for-rtx-spark, or use the guided setup in AGENT-INSTALL.md. This check installs nothing.'
    }

    $blackout = [string](Get-Fact $Facts 'BlackoutEngine' '')
    if ($blackout) {
        $verdicts += New-Verdict -Id 'blackout' -Label 'Blackout guard script' -Kind 'optional' -Detail $blackout
    } else {
        $verdicts += New-Verdict -Id 'blackout' -Label 'Blackout guard script' -Kind 'optional' -Status 'skip' `
            -Detail 'not found; optional, and the rest of the application is unaffected'
    }

    $online = Get-Fact $Facts 'PowerOnline' $null
    if ($null -eq $online) {
        $verdicts += New-Verdict -Id 'power' -Label 'Power source' -Kind 'informational' -Status 'skip' -Detail 'no battery reported'
    } elseif ($online) {
        $verdicts += New-Verdict -Id 'power' -Label 'Power source' -Kind 'informational' -Detail 'AC power; every launch path is available'
    } else {
        $verdicts += New-Verdict -Id 'power' -Label 'Power source' -Kind 'informational' -Status 'warn' `
            -Detail 'on battery; automatic inference launch is refused and a manual start asks first' `
            -Remedy 'Connect power before starting the server. Installing and configuring the application are unaffected.'
    }

    $network = Get-Fact $Facts 'NetworkReachable' $null
    if ($null -eq $network) {
        $verdicts += New-Verdict -Id 'network' -Label 'Network' -Kind 'informational' -Status 'skip' -Detail 'not probed'
    } elseif ($network) {
        $verdicts += New-Verdict -Id 'network' -Label 'Network' -Kind 'informational' `
            -Detail 'api.github.com reachable; release download and update check are available'
    } else {
        $verdicts += New-Verdict -Id 'network' -Label 'Network' -Kind 'informational' -Status 'warn' `
            -Detail 'api.github.com unreachable; offline is supported' `
            -Remedy 'Only the download step needs the network. An offline machine can install a file you brought with you; the update check retries later.'
    }

    $verdicts
}

function Get-ConfigurationVerdicts {
    <#
    .SYNOPSIS
        Judge a settings document against what the application needs to run.

    .DESCRIPTION
        Facts: SettingsPath, SettingsExists, SettingsSchema, ConfigurationError,
        MissingFields, InvalidPaths (objects with Field, Value, Reason),
        BlackoutEngine, BlackoutPython, ApiHost, ApiPort.

        Setup requires five paths: vllm_root, vllm_exe, model_checkpoint,
        launch_config and chat_template. The guard fields are optional by design,
        so a vLLM-only configuration is a valid one.
    #>
    param($Facts)

    $verdicts = @()
    $exists = [bool](Get-Fact $Facts 'SettingsExists' $false)
    $path = [string](Get-Fact $Facts 'SettingsPath' '')
    $configurationError = [string](Get-Fact $Facts 'ConfigurationError' '')

    if (-not $exists) {
        $verdicts += New-Verdict -Id 'settings_present' -Label 'Saved configuration' -Status 'warn' `
            -Detail ("none at {0}; the window opens on the Setup page" -f $path) `
            -Remedy 'Launch the window and complete Setup, or start from CONFIGURATION.example.json.'
    } else {
        $verdicts += New-Verdict -Id 'settings_present' -Label 'Saved configuration' -Detail $path
    }

    if ($configurationError) {
        $verdicts += New-Verdict -Id 'settings_parse' -Label 'Configuration readable' -Status 'fail' `
            -Detail $configurationError `
            -Remedy 'Not valid JSON for schema 2. The application falls back to defaults rather than half-reading it.'
    } elseif ($exists) {
        $schema = [int](Get-Fact $Facts 'SettingsSchema' 0)
        if ($schema -eq 2) {
            $verdicts += New-Verdict -Id 'settings_parse' -Label 'Configuration readable' -Detail 'schema 2'
        } else {
            $verdicts += New-Verdict -Id 'settings_parse' -Label 'Configuration readable' -Kind 'optional' -Status 'warn' `
                -Detail ("schema {0}; the application writes schema 2" -f $schema) `
                -Remedy 'Known keys are kept and the document is rewritten as schema 2 on the next save.'
        }
    }

    $missing = @(Get-Fact $Facts 'MissingFields' @())
    if ($missing.Count -gt 0) {
        $verdicts += New-Verdict -Id 'settings_required' -Label 'Setup paths' -Status 'fail' `
            -Detail ("missing: {0}" -f ($missing -join ', ')) `
            -Remedy 'Set these in Setup. The application refuses to save a set that cannot launch, which is deliberate.'
    } elseif ($exists) {
        $verdicts += New-Verdict -Id 'settings_required' -Label 'Setup paths' `
            -Detail 'vllm_root, vllm_exe, model_checkpoint, launch_config, chat_template all set'
    }

    $invalid = @(Get-Fact $Facts 'InvalidPaths' @())
    if ($invalid.Count -gt 0) {
        $details = ($invalid | ForEach-Object { "{0}: {1}" -f $_.Field, $_.Reason }) -join '; '
        $verdicts += New-Verdict -Id 'settings_paths' -Label 'Configured paths' -Status 'fail' `
            -Detail $details `
            -Remedy 'Paths are recorded, never repaired: correct the path or create the file yourself. Nothing was written here.'
    } elseif ($exists) {
        $verdicts += New-Verdict -Id 'settings_paths' -Label 'Configured paths' -Detail 'every configured path exists and is the right kind'
    }

    $blackoutEngine = [string](Get-Fact $Facts 'BlackoutEngine' '')
    $blackoutPython = [string](Get-Fact $Facts 'BlackoutPython' '')
    if ($blackoutEngine) {
        $detail = $blackoutEngine
        if ($blackoutPython) { $detail += " | interpreter $blackoutPython" }
        $verdicts += New-Verdict -Id 'blackout' -Label 'Blackout guard' -Kind 'optional' -Detail $detail
    } else {
        $verdicts += New-Verdict -Id 'blackout' -Label 'Blackout guard' -Kind 'optional' -Status 'skip' `
            -Detail 'not configured; optional, and the Blackout page says so rather than pretending' `
            -Remedy 'Nothing to install. Point Setup at a blackout.py if you have one.'
    }

    $apiHost = [string](Get-Fact $Facts 'ApiHost' '')
    $apiPort = [int](Get-Fact $Facts 'ApiPort' 0)
    if ($apiHost -and $apiPort -gt 0) {
        $verdicts += New-Verdict -Id 'api_endpoint' -Label 'Inference endpoint' -Kind 'informational' `
            -Detail ("http://{0}:{1}/v1/models is the readiness probe target; nothing was contacted" -f $apiHost, $apiPort)
    }

    $verdicts
}

function Get-InstallVerdicts {
    <#
    .SYNOPSIS
        Judge an installed copy of RTX Spark AI Control, without touching it.

    .DESCRIPTION
        Facts: the platform facts, AppExe, AppExePresent, AppExeSha256,
        KnownInstalledSha256, UninstallKeyPresent, ShortcutPresent,
        ServicePresent, ScheduledTaskPresent, RunKeyPresent, the configuration
        facts, PowerOnline, AutostartInference, VllmProcessCount,
        PythonProcessCount, StartupReport, NetworkReachable, and - only when an
        installer was named - InstallerPath, InstallerSha256 and
        ExpectedInstallerSha256.
    #>
    param($Facts)

    $verdicts = @()
    $verdicts += Get-PlatformVerdicts -Facts $Facts

    $appExe = [string](Get-Fact $Facts 'AppExe' '')
    $appPresent = [bool](Get-Fact $Facts 'AppExePresent' $false)
    if ($appPresent) {
        $verdicts += New-Verdict -Id 'installed' -Label 'Application installed' -Detail $appExe
    } else {
        $verdicts += New-Verdict -Id 'installed' -Label 'Application installed' -Status 'fail' `
            -Detail ("no application found at {0}" -f $appExe) `
            -Remedy 'This check installed and downloaded nothing. Use AGENT-INSTALL.md or INSTALL.md when you want to install, or pass -AppPath for a bare executable.'
    }

    $appSha = [string](Get-Fact $Facts 'AppExeSha256' '')
    $knownShas = @(@(Get-Fact $Facts 'KnownInstalledSha256' @()) | Where-Object { $_ })
    if ($appSha -and $knownShas.Count -gt 0) {
        $matched = $false
        foreach ($known in $knownShas) {
            $comparison = Test-Sha256Match -Expected $known -Actual $appSha
            if ($comparison.Ok) { $matched = $true; break }
        }
        if ($matched) {
            $verdicts += New-Verdict -Id 'app_hash' -Label 'Executable hash' -Kind 'informational' -Detail 'matches a published hash for this version'
        } else {
            $verdicts += New-Verdict -Id 'app_hash' -Label 'Executable hash' -Kind 'informational' -Status 'warn' `
                -Detail ("expected one of {0}, found {1}" -f ($knownShas -join ' or '), $appSha) `
                -Remedy 'A different build from the one in the release manifest. Not necessarily wrong - a local build is not an official artifact - but it is not the file the manifest describes.'
        }
    } elseif ($appSha) {
        $verdicts += New-Verdict -Id 'app_hash' -Label 'Executable hash' -Kind 'informational' `
            -Detail ("{0}; no published hash for this build to compare against" -f $appSha)
    } else {
        $verdicts += New-Verdict -Id 'app_hash' -Label 'Executable hash' -Kind 'informational' -Status 'skip' -Detail 'no executable to hash'
    }

    $uninstall = [bool](Get-Fact $Facts 'UninstallKeyPresent' $false)
    if ($uninstall) {
        $verdicts += New-Verdict -Id 'uninstall_entry' -Label 'Add/Remove Programs entry' -Kind 'optional' -Detail 'present under HKCU'
    } else {
        $verdicts += New-Verdict -Id 'uninstall_entry' -Label 'Add/Remove Programs entry' -Kind 'optional' -Status 'skip' `
            -Detail 'absent; expected for a bare executable, unexpected for a packaged install'
    }

    $shortcut = [bool](Get-Fact $Facts 'ShortcutPresent' $false)
    if ($shortcut) {
        $verdicts += New-Verdict -Id 'shortcut' -Label 'Start Menu shortcut' -Kind 'optional' -Detail 'present'
    } else {
        $verdicts += New-Verdict -Id 'shortcut' -Label 'Start Menu shortcut' -Kind 'optional' -Status 'skip' -Detail 'absent'
    }

    $service = [bool](Get-Fact $Facts 'ServicePresent' $false)
    $task = [bool](Get-Fact $Facts 'ScheduledTaskPresent' $false)
    $runKey = [bool](Get-Fact $Facts 'RunKeyPresent' $false)
    if ($service -or $task -or $runKey) {
        $found = @()
        if ($service) { $found += 'a service' }
        if ($task) { $found += 'a scheduled task' }
        if ($runKey) { $found += 'a Run entry' }
        $verdicts += New-Verdict -Id 'host_footprint' -Label 'Host footprint' -Status 'fail' `
            -Detail ("found {0}" -f ($found -join ', ')) `
            -Remedy 'This application installs none of these. Something else put them there; investigate before trusting the install.'
    } else {
        $verdicts += New-Verdict -Id 'host_footprint' -Label 'Host footprint' -Detail 'no service, no scheduled task, no Run entry - as designed'
    }

    $verdicts += Get-ConfigurationVerdicts -Facts $Facts

    $online = Get-Fact $Facts 'PowerOnline' $null
    $autostart = [bool](Get-Fact $Facts 'AutostartInference' $false)
    if ($null -eq $online) {
        $verdicts += New-Verdict -Id 'power' -Label 'Power source' -Kind 'informational' -Status 'skip' -Detail 'no battery reported'
    } elseif ($online) {
        $verdicts += New-Verdict -Id 'power' -Label 'Power source' -Kind 'informational' -Detail 'AC power'
    } else {
        $verdicts += New-Verdict -Id 'power' -Label 'Power source' -Kind 'informational' -Status 'warn' `
            -Detail 'on battery; automatic inference launch is refused and a manual start asks first' `
            -Remedy 'Designed behaviour, not a defect. Connect power before starting the server.'
    }

    if ($autostart -and $null -ne $online -and -not $online) {
        $verdicts += New-Verdict -Id 'autostart' -Label 'Inference autostart' -Kind 'informational' -Status 'warn' `
            -Detail 'autostart_inference is enabled; it will not be honoured on battery' `
            -Remedy 'The gate lives in the backend, not the interface, so this is the designed behaviour.'
    }

    $vllmCount = [int](Get-Fact $Facts 'VllmProcessCount' 0)
    $pythonCount = [int](Get-Fact $Facts 'PythonProcessCount' 0)
    $detail = 'no server was started, stopped or asked to load weights by this check'
    if ($vllmCount -gt 0) {
        $detail += ("; {0} vllm process(es) were already running and were left alone" -f $vllmCount)
        $verdicts += New-Verdict -Id 'inference' -Label 'Inference untouched' -Kind 'informational' -Status 'warn' -Detail $detail
    } else {
        $detail += ("; {0} python process(es) running" -f $pythonCount)
        $verdicts += New-Verdict -Id 'inference' -Label 'Inference untouched' -Kind 'informational' -Detail $detail
    }

    $startup = Get-Fact $Facts 'StartupReport' $null
    if ($null -ne $startup) {
        $verdicts += New-Verdict -Id 'startup_report' -Label 'Last first paint' -Kind 'informational' `
            -Detail ("{0} ms, version {1}, arch {2}" -f $startup.startup_ms, $startup.version, $startup.arch)
    } else {
        $verdicts += New-Verdict -Id 'startup_report' -Label 'Last first paint' -Kind 'informational' -Status 'skip' `
            -Detail 'no report in %TEMP%; the window has not completed a first paint since that file was last cleared'
    }

    $installerPath = [string](Get-Fact $Facts 'InstallerPath' '')
    $installerSha = [string](Get-Fact $Facts 'InstallerSha256' '')
    $expectedSha = [string](Get-Fact $Facts 'ExpectedInstallerSha256' '')
    if ($installerPath) {
        $match = Test-Sha256Match -Expected $expectedSha -Actual $installerSha
        if (-not $match.Comparable) {
            $shown = $installerSha
            if (-not $shown) { $shown = 'no hash' }
            $verdicts += New-Verdict -Id 'installer_hash' -Label 'Installer checksum' -Status 'warn' `
                -Detail ("{0}: {1}; no expected value to compare against" -f $installerPath, $shown) `
                -Remedy 'Verify against the SHA-256 published with the release you downloaded. An unverifiable installer is a reason to stop.'
        } elseif ($match.Ok) {
            $verdicts += New-Verdict -Id 'installer_hash' -Label 'Installer checksum' -Detail ("{0} matches {1}" -f $installerSha, $installerPath)
        } else {
            $verdicts += New-Verdict -Id 'installer_hash' -Label 'Installer checksum' -Status 'fail' `
                -Detail ("expected {0}, found {1}" -f $match.Expected, $match.Actual) `
                -Remedy 'Do not run this file. Re-download it from the official release and verify again: a mismatch means these bytes are not the published ones.'
        }
    }

    $network = Get-Fact $Facts 'NetworkReachable' $null
    if ($null -eq $network) {
        $verdicts += New-Verdict -Id 'network' -Label 'Network' -Kind 'informational' -Status 'skip' -Detail 'not probed'
    } elseif ($network) {
        $verdicts += New-Verdict -Id 'network' -Label 'Network' -Kind 'informational' -Detail 'api.github.com reachable'
    } else {
        $verdicts += New-Verdict -Id 'network' -Label 'Network' -Kind 'informational' -Status 'warn' `
            -Detail 'api.github.com unreachable; the application works offline and keeps its last cached answer' `
            -Remedy 'No action needed for a local install. The Updates page reports the offline state rather than an error.'
    }

    $verdicts
}

function Get-VerdictCounts {
    param($Verdicts)
    [pscustomobject]@{
        Pass           = @($Verdicts | Where-Object { $_.Status -eq 'pass' }).Count
        Warn           = @($Verdicts | Where-Object { $_.Status -eq 'warn' }).Count
        Fail           = @($Verdicts | Where-Object { $_.Status -eq 'fail' }).Count
        Skip           = @($Verdicts | Where-Object { $_.Status -eq 'skip' }).Count
        FailedRequired = @($Verdicts | Where-Object { $_.Status -eq 'fail' -and $_.Kind -eq 'required' }).Count
    }
}

function Test-UnsupportedPlatform {
    <#
    .SYNOPSIS
        True when a platform check failed. That is a different question from "a
        required check failed" and gets its own exit code.
    #>
    param($Verdicts)
    @($Verdicts | Where-Object { $_.Id -like 'platform_*' -and $_.Status -eq 'fail' }).Count -gt 0
}

function Get-CheckExitCode {
    <#
    .SYNOPSIS
        Collapse verdicts into the documented exit codes.

    .DESCRIPTION
        2 unsupported platform, 3 not installed (only when asked), 1 a required
        check failed, 0 everything required passed. Shared, so the tests assert
        the same arithmetic the scripts use.
    #>
    param($Verdicts, [switch]$NotInstalledIsDistinct)

    if (Test-UnsupportedPlatform -Verdicts $Verdicts) { return 2 }
    if ($NotInstalledIsDistinct) {
        if (@($Verdicts | Where-Object { $_.Id -eq 'installed' -and $_.Status -eq 'fail' }).Count -gt 0) { return 3 }
    }
    $counts = Get-VerdictCounts -Verdicts $Verdicts
    if ($counts.FailedRequired -gt 0) { return 1 }
    return 0
}

function Write-VerdictReport {
    <#
    .SYNOPSIS
        Print verdicts as aligned text on stdout, so a redirect captures them.
    #>
    param(
        [Parameter(Mandatory = $true)]$Verdicts,
        [string]$Title = ''
    )

    if ($Title) {
        Write-Output ''
        Write-Output $Title
        Write-Output ('-' * $Title.Length)
    }

    $statusWidth = 6
    $kindWidth = 13
    foreach ($verdict in $Verdicts) {
        $line = "{0,-$statusWidth} {1,-$kindWidth}  {2}" -f $verdict.Status.ToUpperInvariant(), $verdict.Kind, $verdict.Label
        if ($verdict.Detail) { $line += ": " + $verdict.Detail }
        Write-Output $line
        if ($verdict.Remedy) {
            Write-Output (" " * ($statusWidth + $kindWidth + 2) + "-> " + $verdict.Remedy)
        }
    }
}

function Write-VerdictSummary {
    param($Verdicts)

    $counts = Get-VerdictCounts -Verdicts $Verdicts
    Write-Output ''
    Write-Output ("{0} passed, {1} warnings, {2} failed, {3} not applicable" -f $counts.Pass, $counts.Warn, $counts.Fail, $counts.Skip)

    if ($counts.FailedRequired -gt 0) {
        Write-Output ''
        Write-Output ("{0} required check(s) failed:" -f $counts.FailedRequired)
        foreach ($verdict in @($Verdicts | Where-Object { $_.Status -eq 'fail' -and $_.Kind -eq 'required' })) {
            Write-Output ("  - {0}: {1}" -f $verdict.Label, $verdict.Detail)
        }
    }
}

# --------------------------------------------------------------------- probes
#
# Everything below reads. No probe installs, downloads, writes a registry value,
# changes a driver or power setting, or starts an inference server.

function Get-PlatformFacts {
    <#
    .SYNOPSIS
        Windows build and processor architecture, read from CIM.

    .DESCRIPTION
        Read-only. Falls back to the environment variables only when CIM is
        unavailable, and records which signal answered so the report can say.
    #>
    $facts = @{ OSBuild = 0; OSCaption = ''; Architecture = 'unknown'; ArchitectureSource = '' }

    try {
        $os = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction Stop
        $facts.OSBuild = [int]$os.BuildNumber
        $facts.OSCaption = [string]$os.Caption
    } catch {
        $facts.OSCaption = 'build unknown: ' + $_.Exception.Message
    }

    try {
        $processor = Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop | Select-Object -First 1
        $system = Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop
        $facts.Architecture = ConvertFrom-ArchitectureCode -Code ([int]$processor.Architecture) -SystemType ([string]$system.SystemType)
        $facts.ArchitectureSource = 'Win32_Processor {0}, {1}' -f $processor.Architecture, $system.SystemType
    } catch {
        $facts.Architecture = ConvertFrom-ArchitectureCode -Code (-1) -SystemType ([string]$env:PROCESSOR_ARCHITECTURE)
        $facts.ArchitectureSource = 'environment fallback: PROCESSOR_ARCHITECTURE={0}' -f $env:PROCESSOR_ARCHITECTURE
    }

    $facts
}

function Get-GpuFacts {
    <#
    .SYNOPSIS
        The first GPU nvidia-smi reports, with driver and memory. Read-only.

    .DESCRIPTION
        nvidia-smi is queried, never installed or reconfigured. A machine without
        it reports no GPU rather than guessing from the driver store.
    #>
    $facts = @{ GpuName = ''; DriverVersion = ''; GpuMemoryMiB = 0; NvidiaSmiPath = '' }

    $smi = Get-Command nvidia-smi.exe -ErrorAction SilentlyContinue
    if (-not $smi) { return $facts }
    $facts.NvidiaSmiPath = $smi.Source

    try {
        $lines = @(& $smi.Source '--query-gpu=name,driver_version,memory.total' '--format=csv,noheader' 2>$null)
        foreach ($line in $lines) {
            if ([string]::IsNullOrWhiteSpace($line)) { continue }
            $parts = $line -split ','
            if ($parts.Count -lt 2) { continue }
            $facts.GpuName = $parts[0].Trim()
            $facts.DriverVersion = $parts[1].Trim()
            if ($parts.Count -ge 3) {
                $memory = ($parts[2] -replace '[^0-9]', '')
                if ($memory) { $facts.GpuMemoryMiB = [int]$memory }
            }
            break
        }
    } catch {
        $facts.DriverVersion = ''
    }

    $facts
}

function Get-WebView2Version {
    <#
    .SYNOPSIS
        Installed WebView2 runtime version, from the EdgeUpdate client key or the
        application folder. Read-only; never repairs or installs it.
    #>
    $keys = @(
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
        'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
        'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    )
    foreach ($key in $keys) {
        try {
            if (-not (Test-Path -LiteralPath $key)) { continue }
            $value = (Get-ItemProperty -LiteralPath $key -ErrorAction Stop).pv
            if ($value) { return [string]$value }
        } catch {
            continue
        }
    }

    $folders = @()
    if (${env:ProgramFiles(x86)}) { $folders += (Join-Path ${env:ProgramFiles(x86)} 'Microsoft\EdgeWebView\Application') }
    if ($env:ProgramFiles) { $folders += (Join-Path $env:ProgramFiles 'Microsoft\EdgeWebView\Application') }
    foreach ($folder in $folders) {
        try {
            if (-not (Test-Path -LiteralPath $folder)) { continue }
            $versions = Get-ChildItem -LiteralPath $folder -Directory -ErrorAction Stop |
                Where-Object { $_.Name -match '^\d+\.' } |
                Sort-Object -Property Name -Descending
            if ($versions) { return $versions[0].Name }
        } catch {
            continue
        }
    }

    return ''
}

function Get-FreeSpaceFacts {
    <#
    .SYNOPSIS
        Free space on the installation drive. Read-only.
    #>
    param([string]$Drive = $env:SystemDrive)

    if (-not $Drive) { $Drive = 'C:' }
    $facts = @{ FreeDiskGB = 0; DiskDrive = $Drive }
    try {
        $disk = Get-CimInstance -ClassName Win32_LogicalDisk -Filter ("DeviceID='{0}'" -f $Drive) -ErrorAction Stop
        if ($disk -and $disk.FreeSpace) { $facts.FreeDiskGB = [math]::Round($disk.FreeSpace / 1GB, 1) }
    } catch {
        $facts.FreeDiskGB = 0
    }
    $facts
}

function Read-BuildInfo {
    <#
    .SYNOPSIS
        Read build-info.json from an installation root. Read-only, no import.

    .DESCRIPTION
        The application reads this file for the Updates page; this is the same
        read, parsed the same way. A malformed file reports its error rather than
        an empty version.
    #>
    param([Parameter(Mandatory = $true)][string]$Root)

    $result = @{ Version = ''; ProjectVersion = ''; WheelTag = ''; Error = '' }
    $path = Join-Path $Root 'build-info.json'
    if (-not (Test-Path -LiteralPath $path)) {
        $result.Error = "no build-info.json in $Root"
        return $result
    }
    try {
        $json = Get-Content -LiteralPath $path -Raw -ErrorAction Stop | ConvertFrom-Json
        if ($json.wheel_version) { $result.Version = [string]$json.wheel_version }
        elseif ($json.project_version) { $result.Version = [string]$json.project_version }
        if ($json.project_version) { $result.ProjectVersion = [string]$json.project_version }
        if ($json.wheel_tag) { $result.WheelTag = [string]$json.wheel_tag }
    } catch {
        $result.Error = 'build-info.json is not valid JSON: ' + $_.Exception.Message
    }
    $result
}

function Find-VllmLauncher {
    <#
    .SYNOPSIS
        The vllm.exe inside an installation's Python environment. Read-only.
    #>
    param([Parameter(Mandatory = $true)][string]$Root)

    $relative = @(
        '.venv\Scripts\vllm.exe',
        'venv\Scripts\vllm.exe',
        'Scripts\vllm.exe'
    )
    foreach ($candidate in $relative) {
        $path = Join-Path $Root $candidate
        if (Test-Path -LiteralPath $path) { return $path }
    }
    try {
        $found = Get-ChildItem -LiteralPath $Root -Filter 'vllm.exe' -File -Recurse -Depth 3 -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($found) { return $found.FullName }
    } catch {
        return ''
    }
    return ''
}

function Find-VllmInstallation {
    <#
    .SYNOPSIS
        Locate a native-vLLM installation: the folder holding build-info.json and
        the vllm.exe inside its environment.

    .DESCRIPTION
        -Root checks one tree and is what a caller who knows the paths should use.
        Without it, a bounded walk of %USERPROFILE%, %LOCALAPPDATA% and C:\ runs,
        two levels deep with an entry cap and the skip list the application uses,
        so it stays cheap and offers nothing absurd.

        Read-only throughout: directories are listed, build-info.json is parsed,
        nothing is imported, executed or repaired.
    #>
    param(
        [string]$Root = '',
        [int]$Depth = 2,
        [int]$MaxEntries = 4000
    )

    $result = @{ Root = ''; Exe = ''; Version = ''; WheelTag = ''; Notes = @() }
    $candidates = @()

    if ($Root) {
        if (-not (Test-Path -LiteralPath $Root)) {
            $result.Notes += "the given root does not exist: $Root"
            return $result
        }
        $candidates = @((Resolve-Path -LiteralPath $Root).Path)
    } else {
        $roots = @($env:USERPROFILE, $env:LOCALAPPDATA, 'C:\')
        $skip = @('node_modules', '.git', 'target', 'windows', '$recycle.bin', 'appdata')
        $visited = 0
        $queue = New-Object System.Collections.Queue
        foreach ($candidateRoot in $roots) {
            if (-not $candidateRoot) { continue }
            if (-not (Test-Path -LiteralPath $candidateRoot)) { continue }
            $queue.Enqueue(@{ Path = $candidateRoot; Level = 0 })
        }
        while ($queue.Count -gt 0 -and $visited -lt $MaxEntries) {
            $item = $queue.Dequeue()
            $visited++
            try {
                $entries = Get-ChildItem -LiteralPath $item.Path -Directory -ErrorAction SilentlyContinue
            } catch {
                continue
            }
            foreach ($entry in $entries) {
                if ($visited -ge $MaxEntries) { break }
                $visited++
                $name = $entry.Name.ToLowerInvariant()
                if ($skip -contains $name) { continue }
                if (Test-Path -LiteralPath (Join-Path $entry.FullName 'build-info.json')) {
                    $candidates += $entry.FullName
                }
                if ($item.Level -lt $Depth) {
                    $queue.Enqueue(@{ Path = $entry.FullName; Level = ($item.Level + 1) })
                }
            }
        }
    }

    foreach ($candidate in $candidates) {
        $info = Read-BuildInfo -Root $candidate
        if ($info.Error) {
            $result.Notes += ("{0}: {1}" -f $candidate, $info.Error)
            continue
        }
        $exe = Find-VllmLauncher -Root $candidate
        if ($exe) {
            $result.Root = $candidate
            $result.Exe = $exe
            $result.Version = $info.Version
            $result.WheelTag = $info.WheelTag
            return $result
        }
        $result.Notes += ("{0}: has build-info.json but no vllm.exe in its environment" -f $candidate)
    }

    if ($candidates.Count -eq 0) {
        $result.Notes += 'no folder containing build-info.json was found in the searched roots'
    }
    $result
}

function Find-BlackoutScript {
    <#
    .SYNOPSIS
        Look for an optional blackout.py guard script. Read-only.
    #>
    param([int]$Depth = 2)

    foreach ($root in @($env:USERPROFILE, 'C:\')) {
        if (-not $root) { continue }
        if (-not (Test-Path -LiteralPath $root)) { continue }
        try {
            $found = Get-ChildItem -LiteralPath $root -Filter 'blackout.py' -File -Depth $Depth -ErrorAction SilentlyContinue |
                Select-Object -First 1
            if ($found) { return $found.FullName }
        } catch {
            continue
        }
    }
    return ''
}

function Get-PowerSource {
    <#
    .SYNOPSIS
        $true on AC, $false on battery, $null when the machine reports no battery.

    .DESCRIPTION
        System.Windows.Forms reads the same Windows power API the application
        uses; CIM is the fallback. This reads state and changes no power setting.
    #>
    try {
        Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
        $status = [System.Windows.Forms.SystemInformation]::PowerStatus
        if ($status.BatteryChargeStatus -eq [System.Windows.Forms.BatteryChargeStatus]::NoSystemBattery) {
            return $null
        }
        return ($status.PowerLineStatus -eq [System.Windows.Forms.PowerLineStatus]::Online)
    } catch {
        try {
            $battery = @(Get-CimInstance -ClassName Win32_Battery -ErrorAction Stop)
            if ($battery.Count -eq 0) { return $null }
            return ($battery[0].BatteryStatus -eq 2)
        } catch {
            return $null
        }
    }
}

function Test-EndpointReachable {
    <#
    .SYNOPSIS
        One TCP connect with a short timeout. Offline is a state, not an error.

    .DESCRIPTION
        Nothing is downloaded and no HTTP request is sent; a connect to the
        release host separates "no network" from "network". Failures are
        swallowed on purpose: a probe must never fail a whole verification.
    #>
    param(
        [string]$HostName = 'api.github.com',
        [int]$Port = 443,
        [int]$TimeoutMs = 3000
    )
    $client = $null
    try {
        $client = New-Object System.Net.Sockets.TcpClient
        $task = $client.ConnectAsync($HostName, $Port)
        if (-not $task.Wait($TimeoutMs)) { return $false }
        return $client.Connected
    } catch {
        return $false
    } finally {
        if ($null -ne $client) { $client.Close() }
    }
}

function Get-ProcessCount {
    <#
    .SYNOPSIS
        How many processes carry one of these names. Read-only; starts nothing.
    #>
    param([string[]]$Name = @())

    $total = 0
    foreach ($candidate in $Name) {
        try {
            $total += @(Get-Process -Name $candidate -ErrorAction Stop).Count
        } catch {
            continue
        }
    }
    return $total
}
