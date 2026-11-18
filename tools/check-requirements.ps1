<#
.SYNOPSIS
    Read-only check that this machine can run RTX Spark AI Control.

.DESCRIPTION
    Reports what this machine has and what it is missing, in the operator's
    words, and then stops. It is the first thing to run on a new machine and the
    thing to attach to a support request when something does not start.

    Read-only means exactly that. This script installs no package, downloads
    nothing, changes no driver, CUDA component or power setting, writes no file
    and starts no inference server. The single network touch is one TCP connect
    to api.github.com to tell "offline" from "unreachable", and `-SkipNetwork`
    removes even that.

    The checks are: Windows 11, ARM64, an NVIDIA GPU and driver, the WebView2
    runtime, free space, an optionally configured Python, a native-vLLM
    installation, the optional blackout guard, the power source and reachability.
    A missing native-vLLM is a warning because the window runs without one; pass
    `-RequireInference` to make it a failure when the machine is meant to serve.

.PARAMETER VllmRoot
    Check this installation instead of discovering one. The folder must contain
    build-info.json and an environment holding vllm.exe.

.PARAMETER ModelCheckpoint
    Check this model folder when given. Nothing is read inside it beyond its
    existence.

.PARAMETER MinimumFreeGB
    Free space the installation drive should have. Default 20. This is working
    room, not model storage.

.PARAMETER RequireInference
    Treat a missing native-vLLM installation as a failure.

.PARAMETER SkipDiscovery
    Do not search the disk for a native-vLLM installation or a blackout script.

.PARAMETER SkipNetwork
    Do not probe for network reachability.

.PARAMETER Json
    Emit one JSON document instead of the aligned report.

.EXAMPLE
    powershell -File tools\check-requirements.ps1

.EXAMPLE
    powershell -File tools\check-requirements.ps1 -RequireInference -VllmRoot C:\AI\native-vLLM

.NOTES
    Exit codes: 0 every required check passed, 1 a required check failed,
    2 unsupported platform (not Windows 11 ARM64), 3 the checks themselves could
    not run.
#>
param(
    [string]$VllmRoot = '',
    [string]$ModelCheckpoint = '',
    [double]$MinimumFreeGB = 20,
    [switch]$RequireInference,
    [switch]$SkipDiscovery,
    [switch]$SkipNetwork,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'lib\install-checks.ps1')

$notes = @()
try {
    $facts = @{}
    $facts += Get-PlatformFacts
    $facts += Get-GpuFacts
    $facts += Get-FreeSpaceFacts
    $facts.WebView2Version = Get-WebView2Version

    $python = Get-Command python.exe -ErrorAction SilentlyContinue
    if ($python) { $facts.PythonPath = $python.Source }

    if ($VllmRoot) {
        $vllm = Find-VllmInstallation -Root $VllmRoot
    } elseif (-not $SkipDiscovery) {
        $vllm = Find-VllmInstallation
    } else {
        $vllm = @{ Root = ''; Exe = ''; Version = ''; Notes = @() }
    }
    $facts.VllmRoot = $vllm.Root
    $facts.VllmExe = $vllm.Exe
    $facts.VllmVersion = $vllm.Version
    $notes += @($vllm.Notes)

    if ($SkipDiscovery) {
        $facts.BlackoutEngine = ''
    } elseif ($env:V2_TEST_BLACKOUT_ENGINE) {
        $facts.BlackoutEngine = $env:V2_TEST_BLACKOUT_ENGINE
    } else {
        $facts.BlackoutEngine = Find-BlackoutScript
    }

    $facts.PowerOnline = Get-PowerSource
    if (-not $SkipNetwork) { $facts.NetworkReachable = Test-EndpointReachable }
} catch {
    Write-Output ('the checks could not run: ' + $_.Exception.Message)
    exit 3
}

$verdicts = @(Get-RequirementVerdicts -Facts $facts -MinimumFreeGB $MinimumFreeGB -RequireInference:$RequireInference)

if ($ModelCheckpoint) {
    if (Test-Path -LiteralPath $ModelCheckpoint) {
        $verdicts += New-Verdict -Id 'model_checkpoint' -Label 'Model checkpoint' -Detail $ModelCheckpoint
    } else {
        $verdicts += New-Verdict -Id 'model_checkpoint' -Label 'Model checkpoint' -Status 'fail' `
            -Detail ("{0} does not exist" -f $ModelCheckpoint) `
            -Remedy 'Point Setup at the checkpoint you actually have. Nothing was created or downloaded.'
    }
}

$exitCode = Get-CheckExitCode -Verdicts $verdicts

if ($Json) {
    $payload = [ordered]@{
        script   = 'check-requirements'
        machine  = [ordered]@{
            build        = [int](Get-Fact $facts 'OSBuild' 0)
            caption      = [string](Get-Fact $facts 'OSCaption' '')
            architecture = [string](Get-Fact $facts 'Architecture' 'unknown')
            gpu          = [string](Get-Fact $facts 'GpuName' '')
            driver       = [string](Get-Fact $facts 'DriverVersion' '')
            powerOnline  = Get-Fact $facts 'PowerOnline' $null
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

Write-Output 'RTX Spark AI Control - requirement check (read-only)'
Write-Output 'Nothing below installed, downloaded, started or changed anything.'
Write-VerdictReport -Verdicts $verdicts -Title 'Checks'

foreach ($note in @($notes | Where-Object { $_ })) {
    Write-Output ('note: ' + $note)
}

Write-VerdictSummary -Verdicts $verdicts

$message = switch ($exitCode) {
    0 { 'Ready: every required check passed. Manual install steps are in INSTALL.md.' }
    1 { 'Not ready: a required check failed. Each failure above says what is missing and what not to expect from this script.' }
    2 { 'Unsupported platform: RTX Spark AI Control v0.1.0 targets Windows 11 ARM64 only.' }
    default { 'The checks could not run.' }
}
Write-Output ''
Write-Output $message
exit $exitCode
