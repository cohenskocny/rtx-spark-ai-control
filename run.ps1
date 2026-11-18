# Launch RTX Spark AI Control V2 (release build, native ARM64).
# Nothing is installed, registered, or started alongside it.
param([switch]$Build)

$exe = Join-Path $PSScriptRoot 'src-tauri\target\release\rtx-spark-ai-control-v2.exe'

if ($Build -or -not (Test-Path $exe)) {
    # Must go through the Tauri CLI: a bare `cargo build` links fine but carries
    # the dev URL (http://localhost:1420) instead of the embedded frontend, and
    # the window comes up black.
    Write-Host "Building (vite + tauri)..." -ForegroundColor Cyan
    Push-Location $PSScriptRoot
    npm run app:build
    $code = $LASTEXITCODE
    Pop-Location
    if ($code -ne 0) { exit $code }
}

Start-Process -FilePath $exe
Write-Host "RTX Spark AI Control V2 started."
