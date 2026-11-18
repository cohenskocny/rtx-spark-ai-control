<#
.SYNOPSIS
    Scenario tests for the read-only installer tooling.

.DESCRIPTION
    Feeds fabricated fact sets to the pure checks in tools/lib/install-checks.ps1
    and asserts the verdicts, so the awkward cases are covered without a second
    machine, a GPU, a real installer or a network connection.

    The scenarios are the ones the documentation promises: x64 Windows, Windows
    10, a missing driver, a missing WebView2 runtime, a machine without
    native-vLLM, an existing native-vLLM installation, a missing optional
    blackout engine, a battery-powered laptop, an offline host, an installer
    checksum mismatch, and a user who declines to install.

    It finishes with an end-to-end run of both scripts in -Json mode on this
    machine, which proves they execute rather than merely parse. Those runs are
    read-only: no inference is started and no model is loaded.

.EXAMPLE
    powershell -File tools\test-install-scripts.ps1

.EXAMPLE
    powershell -File tools\test-install-scripts.ps1 -SkipIntegration

.NOTES
    Exit codes: 0 every scenario passed, 1 a scenario failed.
#>
param(
    [switch]$SkipIntegration
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'lib\install-checks.ps1')

$script:passed = 0
$script:failed = @()

function Test-Case {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Body
    )
    try {
        & $Body
        $script:passed++
        Write-Output ('PASS  ' + $Name)
    } catch {
        $script:failed += ("{0}: {1}" -f $Name, $_.Exception.Message)
        Write-Output ('FAIL  ' + $Name + ' - ' + $_.Exception.Message)
    }
}

function Assert-True {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw $Message }
}

function Assert-Verdict {
    param(
        $Verdicts,
        [Parameter(Mandatory = $true)][string]$Id,
        [Parameter(Mandatory = $true)][string]$Status,
        [string]$DetailLike = ''
    )
    $found = @($Verdicts | Where-Object { $_.Id -eq $Id })
    Assert-True ($found.Count -eq 1) ("expected exactly one verdict with id '$Id', found {0}" -f $found.Count)
    Assert-True ($found[0].Status -eq $Status) ("'{0}' was '{1}', expected '{2}' ({3})" -f $Id, $found[0].Status, $Status, $found[0].Detail)
    if ($DetailLike) {
        Assert-True ($found[0].Detail -like $DetailLike) ("'{0}' detail was '{1}', expected it to match '{2}'" -f $Id, $found[0].Detail, $DetailLike)
    }
}

function Assert-ExitCode {
    param($Verdicts, [int]$Expected, [switch]$NotInstalled)
    $actual = Get-CheckExitCode -Verdicts $Verdicts -NotInstalledIsDistinct:$NotInstalled
    Assert-True ($actual -eq $Expected) ("exit code was {0}, expected {1}" -f $actual, $Expected)
}

function New-SupportedFacts {
    @{
        OSBuild            = 28000
        OSCaption          = 'Microsoft Windows 11 Pro'
        Architecture       = 'ARM64'
        ArchitectureSource = 'Win32_Processor 12, ARM64-based PC'
        GpuName            = 'NVIDIA RTX Spark N1X'
        DriverVersion      = '621.14'
        GpuMemoryMiB       = 98304
        WebView2Version    = '157.0.4341.60'
        FreeDiskGB         = 120.0
        DiskDrive          = 'C:'
        PythonPath         = 'C:\Python313\python.exe'
        VllmRoot           = 'C:\AI\native-vLLM'
        VllmExe            = 'C:\AI\native-vLLM\.venv\Scripts\vllm.exe'
        VllmVersion        = '0.32.0+rtx_spark.0.2.0'
        BlackoutEngine     = 'C:\AI\blackout\blackout.py'
        PowerOnline        = $true
        NetworkReachable   = $true
    }
}

function New-ConfiguredFacts {
    $facts = New-SupportedFacts
    $facts.AppExe = 'C:\Users\someone\AppData\Local\RTX Spark AI Control\rtx-spark-ai-control-v2.exe'
    $facts.AppExePresent = $true
    $facts.AppExeSha256 = 'dcba0ea3804bcb1adf25fc188b564e5b98311dc4a6e21a88dbe56773618d63b0'
    $facts.KnownInstalledSha256 = @(
        'dcba0ea3804bcb1adf25fc188b564e5b98311dc4a6e21a88dbe56773618d63b0',
        'ee57faabd6047a4e38e13e0e6f2cac7ed0e3f5a1d28721b1d6f1340ddae43990'
    )
    $facts.UninstallKeyPresent = $true
    $facts.ShortcutPresent = $true
    $facts.ServicePresent = $false
    $facts.ScheduledTaskPresent = $false
    $facts.RunKeyPresent = $false
    $facts.SettingsPath = 'C:\Users\someone\AppData\Roaming\com.rtxspark.aicontrol.v2\settings.json'
    $facts.SettingsExists = $true
    $facts.SettingsSchema = 2
    $facts.MissingFields = @()
    $facts.InvalidPaths = @()
    $facts.BlackoutEngine = 'C:\AI\blackout\blackout.py'
    $facts.BlackoutPython = 'C:\Python313\pythonw.exe'
    $facts.ApiHost = '127.0.0.1'
    $facts.ApiPort = 8000
    $facts.AutostartInference = $false
    $facts.VllmProcessCount = 0
    $facts.PythonProcessCount = 2
    $facts.StartupReport = [pscustomobject]@{ startup_ms = 812; version = '0.1.0'; arch = 'aarch64' }
    $facts
}

Test-Case 'ARM64 is read from the CIM code, not the emulated process architecture' {
    Assert-True ((ConvertFrom-ArchitectureCode -Code 12) -eq 'ARM64') 'code 12 must be ARM64'
    Assert-True ((ConvertFrom-ArchitectureCode -Code 9 -SystemType 'x64-based PC') -eq 'AMD64') 'code 9 must be AMD64'
    Assert-True ((ConvertFrom-ArchitectureCode -Code 0) -eq 'x86') 'code 0 must be x86'
    Assert-True ((ConvertFrom-ArchitectureCode -Code -1 -SystemType 'ARM64-based PC') -eq 'ARM64') 'the system type is the fallback'
    Assert-True ((ConvertFrom-ArchitectureCode -Code -1 -SystemType 'x64-based PC') -eq 'unknown') 'an unknown code must not be guessed'
}

Test-Case 'Windows 11 ARM64 passes every required check' {
    $verdicts = @(Get-RequirementVerdicts -Facts (New-SupportedFacts))
    Assert-Verdict $verdicts 'platform_arch' 'pass'
    Assert-Verdict $verdicts 'gpu' 'pass'
    Assert-Verdict $verdicts 'webview2' 'pass'
    Assert-Verdict $verdicts 'vllm' 'pass'
    Assert-ExitCode $verdicts 0
}

Test-Case 'x64 Windows is unsupported, not merely broken' {
    $facts = New-SupportedFacts
    $facts.Architecture = 'AMD64'
    $facts.ArchitectureSource = 'Win32_Processor 9'
    $verdicts = @(Get-RequirementVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'platform_arch' 'fail'
    Assert-True (@($verdicts | Where-Object { $_.Id -eq 'platform_arch' })[0].Remedy -like '*ARM64-only*') 'the remedy must say the package is ARM64-only'
    Assert-ExitCode $verdicts 2
}

Test-Case 'Windows 10 is unsupported' {
    $facts = New-SupportedFacts
    $facts.OSBuild = 19045
    $facts.OSCaption = 'Microsoft Windows 10 Pro'
    $verdicts = @(Get-RequirementVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'platform_os' 'fail'
    Assert-ExitCode $verdicts 2
}

Test-Case 'a missing GPU fails, and nothing claims it will be installed' {
    $facts = New-SupportedFacts
    $facts.GpuName = ''
    $facts.DriverVersion = ''
    $verdicts = @(Get-RequirementVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'gpu' 'fail'
    Assert-Verdict $verdicts 'driver' 'warn'
    Assert-True (@($verdicts | Where-Object { $_.Id -eq 'gpu' })[0].Remedy -like '*installs and changes no driver*') 'the remedy must disclaim installing a driver'
    Assert-ExitCode $verdicts 1
}

Test-Case 'a missing WebView2 runtime fails' {
    $facts = New-SupportedFacts
    $facts.WebView2Version = ''
    $verdicts = @(Get-RequirementVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'webview2' 'fail'
    Assert-ExitCode $verdicts 1
}

Test-Case 'too little free space fails' {
    $facts = New-SupportedFacts
    $facts.FreeDiskGB = 4.5
    $verdicts = @(Get-RequirementVerdicts -Facts $facts -MinimumFreeGB 20)
    Assert-Verdict $verdicts 'disk' 'fail'
    Assert-ExitCode $verdicts 1
}

Test-Case 'a machine without native-vLLM still supports the GUI' {
    $facts = New-SupportedFacts
    $facts.VllmRoot = ''
    $facts.VllmExe = ''
    $facts.VllmVersion = ''
    $verdicts = @(Get-RequirementVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'vllm' 'warn'
    Assert-ExitCode $verdicts 0
    $strict = @(Get-RequirementVerdicts -Facts $facts -RequireInference)
    Assert-Verdict $strict 'vllm' 'fail'
    Assert-ExitCode $strict 1
}

Test-Case 'an existing native-vLLM installation is reported with its version' {
    $verdicts = @(Get-RequirementVerdicts -Facts (New-SupportedFacts))
    Assert-Verdict $verdicts 'vllm' 'pass' -DetailLike '*0.32.0+rtx_spark.0.2.0*'
}

Test-Case 'an incomplete vLLM configuration is a failure' {
    $facts = New-SupportedFacts
    $facts.VllmExe = ''
    $verdicts = @(Get-RequirementVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'vllm' 'fail'
    Assert-ExitCode $verdicts 1
}

Test-Case 'a missing optional blackout engine is skipped, never failed' {
    $facts = New-SupportedFacts
    $facts.BlackoutEngine = ''
    $verdicts = @(Get-RequirementVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'blackout' 'skip'
    Assert-ExitCode $verdicts 0
}

Test-Case 'a battery-powered machine warns and still passes' {
    $facts = New-SupportedFacts
    $facts.PowerOnline = $false
    $verdicts = @(Get-RequirementVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'power' 'warn'
    Assert-ExitCode $verdicts 0
}

Test-Case 'an offline machine warns and still passes' {
    $facts = New-SupportedFacts
    $facts.NetworkReachable = $false
    $verdicts = @(Get-RequirementVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'network' 'warn'
    Assert-ExitCode $verdicts 0
}

Test-Case 'a machine with no battery reports the power source as unknown' {
    $facts = New-SupportedFacts
    $facts.PowerOnline = $null
    $verdicts = @(Get-RequirementVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'power' 'skip'
    Assert-ExitCode $verdicts 0
}

Test-Case 'hash comparison tolerates case and whitespace and never invents a mismatch' {
    $upper = 'DCBA0EA3804BCB1ADF25FC188B564E5B98311DC4A6E21A88DBE56773618D63B0'
    $padded = " dcba0ea3804bcb1adf25fc188b564e5b98311dc4a6e21a88dbe56773618d63b0`r`n"
    Assert-True ((Test-Sha256Match -Expected $upper -Actual $padded).Ok) 'these are the same hash'
    $mismatch = Test-Sha256Match -Expected $upper -Actual 'ee57faabd6047a4e38e13e0e6f2cac7ed0e3f5a1d28721b1d6f1340ddae43990'
    Assert-True ($mismatch.Comparable -and -not $mismatch.Ok) 'different hashes must not match'
    Assert-True (-not (Test-Sha256Match -Expected '' -Actual $upper).Comparable) 'an absent expectation is not a mismatch'
}

Test-Case 'an installer checksum mismatch stops the verification' {
    $facts = New-ConfiguredFacts
    $facts.InstallerPath = 'C:\Downloads\RTX Spark AI Control_0.1.0_arm64-setup.exe'
    $facts.InstallerSha256 = '0000000000000000000000000000000000000000000000000000000000000000'
    $facts.ExpectedInstallerSha256 = '39ec4b17a2baca830f930994889084e35e8793abeb18306cf25f3acb584503f6'
    $verdicts = @(Get-InstallVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'installer_hash' 'fail' -DetailLike '*expected 39ec4b17*'
    Assert-True (@($verdicts | Where-Object { $_.Id -eq 'installer_hash' })[0].Remedy -like '*Do not run this file*') 'the remedy must tell the operator not to run it'
    Assert-ExitCode $verdicts 1 -NotInstalled
}

Test-Case 'a matching installer checksum passes' {
    $facts = New-ConfiguredFacts
    $facts.InstallerPath = 'C:\Downloads\RTX Spark AI Control_0.1.0_arm64-setup.exe'
    $facts.InstallerSha256 = '39EC4B17A2BACA830F930994889084E35E8793ABEB18306CF25F3ACB584503F6'
    $facts.ExpectedInstallerSha256 = '39ec4b17a2baca830f930994889084e35e8793abeb18306cf25f3acb584503f6'
    $verdicts = @(Get-InstallVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'installer_hash' 'pass'
    Assert-ExitCode $verdicts 0 -NotInstalled
}

Test-Case 'an installer with no published hash is a warning, not a pass' {
    $facts = New-ConfiguredFacts
    $facts.InstallerPath = 'C:\Downloads\unknown-setup.exe'
    $facts.InstallerSha256 = '39ec4b17a2baca830f930994889084e35e8793abeb18306cf25f3acb584503f6'
    $facts.ExpectedInstallerSha256 = ''
    $verdicts = @(Get-InstallVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'installer_hash' 'warn'
}

Test-Case 'a configured installation verifies end to end' {
    $verdicts = @(Get-InstallVerdicts -Facts (New-ConfiguredFacts))
    Assert-Verdict $verdicts 'installed' 'pass'
    Assert-Verdict $verdicts 'app_hash' 'pass'
    Assert-Verdict $verdicts 'host_footprint' 'pass'
    Assert-Verdict $verdicts 'settings_required' 'pass'
    Assert-Verdict $verdicts 'settings_paths' 'pass'
    Assert-Verdict $verdicts 'blackout' 'pass'
    Assert-ExitCode $verdicts 0 -NotInstalled
}

Test-Case 'a user who declines to install gets a clear answer and no action' {
    $facts = New-ConfiguredFacts
    $facts.AppExePresent = $false
    $facts.AppExeSha256 = ''
    $facts.KnownInstalledSha256 = @()
    $facts.UninstallKeyPresent = $false
    $facts.ShortcutPresent = $false
    $facts.SettingsExists = $false
    $facts.SettingsSchema = 0
    $facts.MissingFields = @()
    $facts.InvalidPaths = @()
    $facts.BlackoutEngine = ''
    $facts.BlackoutPython = ''
    $verdicts = @(Get-InstallVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'installed' 'fail'
    $installed = @($verdicts | Where-Object { $_.Id -eq 'installed' })[0]
    Assert-True ($installed.Remedy -like '*installed and downloaded nothing*') 'the remedy must state that nothing was installed'
    Assert-True ($installed.Remedy -like '*AGENT-INSTALL.md*') 'the remedy must point at the installation guide'
    Assert-ExitCode $verdicts 3 -NotInstalled
    Assert-True (-not (Test-UnsupportedPlatform -Verdicts $verdicts)) 'an unsupported-platform answer would hide the real state'
}

Test-Case 'a vLLM-only configuration verifies with the blackout engine absent' {
    $facts = New-ConfiguredFacts
    $facts.BlackoutEngine = ''
    $facts.BlackoutPython = ''
    $verdicts = @(Get-InstallVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'blackout' 'skip'
    Assert-ExitCode $verdicts 0 -NotInstalled
}

Test-Case 'a configuration missing Setup paths fails with the names' {
    $facts = New-ConfiguredFacts
    $facts.MissingFields = @('model_checkpoint', 'chat_template')
    $verdicts = @(Get-InstallVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'settings_required' 'fail' -DetailLike '*model_checkpoint*'
    Assert-ExitCode $verdicts 1 -NotInstalled
}

Test-Case 'a configured path that does not exist fails' {
    $facts = New-ConfiguredFacts
    $facts.InvalidPaths = @(@{ Field = 'model_checkpoint'; Value = 'C:\models\gone'; Reason = 'does not exist' })
    $verdicts = @(Get-InstallVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'settings_paths' 'fail' -DetailLike '*does not exist*'
    Assert-ExitCode $verdicts 1 -NotInstalled
}

Test-Case 'an unreadable configuration is a failure, not a silent default' {
    $facts = New-ConfiguredFacts
    $facts.ConfigurationError = 'Invalid JSON: unexpected end of input'
    $verdicts = @(Get-InstallVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'settings_parse' 'fail'
    Assert-ExitCode $verdicts 1 -NotInstalled
}

Test-Case 'a service or scheduled task is a failure: this application installs neither' {
    $facts = New-ConfiguredFacts
    $facts.ServicePresent = $true
    $verdicts = @(Get-InstallVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'host_footprint' 'fail' -DetailLike '*service*'
    Assert-ExitCode $verdicts 1 -NotInstalled
}

Test-Case 'an executable hash from another build warns without failing' {
    $facts = New-ConfiguredFacts
    $facts.AppExeSha256 = '1111111111111111111111111111111111111111111111111111111111111111'
    $verdicts = @(Get-InstallVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'app_hash' 'warn' -DetailLike '*expected one of*'
    Assert-ExitCode $verdicts 0 -NotInstalled
}

Test-Case 'an inference server that is already running is left alone' {
    $facts = New-ConfiguredFacts
    $facts.VllmProcessCount = 1
    $verdicts = @(Get-InstallVerdicts -Facts $facts)
    Assert-Verdict $verdicts 'inference' 'warn' -DetailLike '*left alone*'
    Assert-ExitCode $verdicts 0 -NotInstalled
}

Test-Case 'the installation guide keeps the safety wording an agent must honour' {
    $guide = Join-Path (Split-Path -Parent $PSScriptRoot) 'AGENT-INSTALL.md'
    Assert-True (Test-Path -LiteralPath $guide) 'AGENT-INSTALL.md must exist'
    $text = Get-Content -LiteralPath $guide -Raw
    foreach ($phrase in @('SHA-256', 'approval', 'decline', 'Windows 11 ARM64', 'devjeonghwan/vllm-for-rtx-spark', 'cohenskocny/rtx-spark-ai-control')) {
        Assert-True ($text -like ('*' + $phrase + '*')) ("the guide must mention '{0}'" -f $phrase)
    }
    Assert-True ($text -like '*does not install*' -or $text -like '*Do not install*') 'the guide must scope what the agent may install'
}

Test-Case 'the repository explains the scripts to a coding agent' {
    $agents = Join-Path (Split-Path -Parent $PSScriptRoot) 'AGENTS.md'
    Assert-True (Test-Path -LiteralPath $agents) 'AGENTS.md must exist'
    $text = Get-Content -LiteralPath $agents -Raw
    foreach ($phrase in @('check-requirements.ps1', 'verify-install.ps1', 'Windows 11 ARM64', 'Safety')) {
        Assert-True ($text -like ('*' + $phrase + '*')) ("AGENTS.md must mention '{0}'" -f $phrase)
    }
}

if (-not $SkipIntegration) {
    Test-Case 'both scripts execute end to end on this machine, in JSON' {
        $requirements = Join-Path $PSScriptRoot 'check-requirements.ps1'
        $verify = Join-Path $PSScriptRoot 'verify-install.ps1'

        $requirementText = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $requirements -SkipDiscovery -SkipNetwork -Json 2>&1
        $requirementCode = $LASTEXITCODE
        Assert-True (@(0, 1, 2, 3) -contains $requirementCode) ("check-requirements.ps1 exited {0}; expected 0, 1, 2 or 3" -f $requirementCode)
        $requirementJson = ($requirementText -join "`n") | ConvertFrom-Json
        Assert-True ($requirementJson.script -eq 'check-requirements') 'check-requirements must identify itself in JSON'
        Assert-True (@($requirementJson.verdicts).Count -ge 6) 'check-requirements must report its verdicts'
        Assert-True ($requirementJson.exitCode -eq $requirementCode) 'the JSON exit code must match the process exit code'

        $verifyText = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $verify -SkipNetwork -Json 2>&1
        $verifyCode = $LASTEXITCODE
        Assert-True (@(0, 1, 2, 3) -contains $verifyCode) ("verify-install.ps1 exited {0}; expected 0, 1, 2 or 3" -f $verifyCode)
        $verifyJson = ($verifyText -join "`n") | ConvertFrom-Json
        Assert-True ($verifyJson.script -eq 'verify-install') 'verify-install must identify itself in JSON'
        Assert-True (@($verifyJson.verdicts | Where-Object { $_.id -eq 'installed' }).Count -eq 1) 'verify-install must report the installed state'
        Assert-True ($verifyJson.exitCode -eq $verifyCode) 'the JSON exit code must match the process exit code'
    }
}

Write-Output ''
Write-Output ("{0} scenario(s) passed, {1} failed" -f $script:passed, $script:failed.Count)
if ($script:failed.Count -gt 0) {
    foreach ($failure in $script:failed) { Write-Output ('  - ' + $failure) }
    exit 1
}
exit 0
