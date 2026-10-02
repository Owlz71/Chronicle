[CmdletBinding()]
param(
    [ValidateSet("exe", "bundle")]
    [string]$Mode = "exe",

    [switch]$Run,

    [switch]$StopRunning
)

$ErrorActionPreference = "Stop"

# VS Code (or any long-running shell) may have started before CARGO_HOME and
# RUSTUP_HOME were persisted, so fall back to the user-level values instead of
# letting rustup silently use %USERPROFILE%\.rustup and re-download the toolchain.
foreach ($name in @("CARGO_HOME", "RUSTUP_HOME")) {
    if (-not [Environment]::GetEnvironmentVariable($name, "Process")) {
        $persisted = [Environment]::GetEnvironmentVariable($name, "User")
        if ($persisted) { Set-Item -Path "Env:$name" -Value $persisted }
    }
}
if (-not $env:CARGO_HOME -or -not $env:RUSTUP_HOME) {
    throw "CARGO_HOME and RUSTUP_HOME are not set. Install Rust first, or set both variables and retry."
}
$cargoBin = Join-Path $env:CARGO_HOME "bin"
if ((Test-Path $cargoBin) -and (($env:PATH -split ';') -notcontains $cargoBin)) {
    $env:PATH = "$cargoBin;$env:PATH"
}

# A running Chronicle keeps a lock on the built executable, which makes tauri
# fail with a confusing "failed to rename app binary (os error 5)" message.
$running = @(Get-Process -Name "Chronicle" -ErrorAction SilentlyContinue)
if ($running.Count -gt 0) {
    $runningIds = ($running | Select-Object -ExpandProperty Id) -join ", "
    if (-not $StopRunning) {
        throw "Chronicle is running (PID $runningIds) and locks the executable. Quit it from the tray icon, or re-run with -StopRunning."
    }
    Write-Host "[stop] Closing Chronicle (PID $runningIds)..." -ForegroundColor Yellow
    $running | Stop-Process -Force -ErrorAction SilentlyContinue
    $deadline = (Get-Date).AddSeconds(10)
    while ((Get-Process -Name "Chronicle" -ErrorAction SilentlyContinue) -and ((Get-Date) -lt $deadline)) {
        Start-Sleep -Milliseconds 300
    }
    if (Get-Process -Name "Chronicle" -ErrorAction SilentlyContinue) {
        throw "Chronicle is still running and could not be closed automatically."
    }
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$desktopDirectory = Join-Path $repositoryRoot "apps/desktop"
$exePath = Join-Path $repositoryRoot "apps/desktop/src-tauri/target/release/Chronicle.exe"
$portableDirectory = Join-Path $repositoryRoot "apps/desktop/src-tauri/target/release/bundle/portable"
$outputDirectory = Join-Path $repositoryRoot "build"

if (-not (Test-Path (Join-Path $desktopDirectory "node_modules"))) {
    Write-Host "[setup] Installing desktop dependencies (npm ci)..." -ForegroundColor Cyan
    & npm --prefix $desktopDirectory ci
    if ($LASTEXITCODE -ne 0) { throw "npm ci failed with exit code $LASTEXITCODE." }
}

$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()

if ($Mode -eq "bundle") {
    Write-Host "[1/3] Building executable and installer..." -ForegroundColor Cyan
    & npm --prefix $desktopDirectory run desktop:build
    if ($LASTEXITCODE -ne 0) { throw "desktop:build failed with exit code $LASTEXITCODE." }

    Write-Host "[2/3] Packaging the portable edition..." -ForegroundColor Cyan
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "package-portable.ps1") `
        -ExecutablePath $exePath -OutputDirectory $portableDirectory
    if ($LASTEXITCODE -ne 0) { throw "Portable packaging failed with exit code $LASTEXITCODE." }

    $version = (Get-Content (Join-Path $desktopDirectory "package.json") -Raw | ConvertFrom-Json).version
    Write-Host "[3/3] Release artifacts:" -ForegroundColor Cyan
    Write-Host ("  installer : {0}" -f (Join-Path $repositoryRoot "apps/desktop/src-tauri/target/release/bundle/nsis/Chronicle_${version}_x64-setup.exe"))
    Write-Host ("  portable  : {0}" -f (Join-Path $portableDirectory "Chronicle-${version}-windows-x64-portable.zip"))
} else {
    Write-Host "[1/2] Building executable (no installer)..." -ForegroundColor Cyan
    & npm --prefix $desktopDirectory run desktop:build -- --no-bundle
    if ($LASTEXITCODE -ne 0) { throw "desktop:build failed with exit code $LASTEXITCODE." }
}

if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) {
    throw "The build reported success but $exePath does not exist."
}

# Copy next to the repository root so the executable is easy to find and
# double-click without digging through target\release.
Write-Host "[copy] Staging the executable for testing..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$outputExe = Join-Path $outputDirectory "Chronicle.exe"
Copy-Item -LiteralPath $exePath -Destination $outputExe -Force

$stopwatch.Stop()
Write-Host ""
Write-Host ("Done in {0:n1}s" -f $stopwatch.Elapsed.TotalSeconds) -ForegroundColor Green
Write-Host ("  built     : {0}" -f $exePath)
Write-Host ("  ready to  : {0}" -f $outputExe)

if ($Run) {
    Write-Host "[run] Launching Chronicle..." -ForegroundColor Cyan
    Start-Process -FilePath $outputExe
}
