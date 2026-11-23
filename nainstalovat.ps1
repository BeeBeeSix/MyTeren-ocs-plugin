# Installs the Terrain plugin into Open CAD Studio (version according to release – see README).
# Launched via nainstalovat.bat (double-click).
#
# If a pre-built plugin (teren_plugin.dll) is next to the script, it is simply copied.
# Otherwise, it is built from source – this requires Rust (https://rustup.rs);
# the Rust version does not matter.

$ErrorActionPreference = "Stop"
Set-Location -Path $PSScriptRoot

function EndScript($code) {
    Write-Host ""
    Read-Host "Press Enter to close the window"
    exit $code
}

if ($PSScriptRoot -like "$env:TEMP*") {
    Write-Host "It seems the installation is running directly from a zip file. Please extract the zip first (right-click › Extract All) and run nainstalovat.bat from the extracted folder." -ForegroundColor Yellow
    EndScript 1
}

$dll = Join-Path $PSScriptRoot "teren_plugin.dll"
if (-not (Test-Path $dll)) {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        $path = Join-Path $env:USERPROFILE ".cargo\bin"
        if (Test-Path (Join-Path $path "cargo.exe")) {
            $env:PATH = "$path;$env:PATH"
        } else {
            Write-Host "ERROR: Pre-built plugin (teren_plugin.dll) is missing, and Rust (cargo) is not available to build it." -ForegroundColor Red
            Write-Host "Download the zip with the pre-built plugin, or install Rust from https://rustup.rs" -ForegroundColor Red
            EndScript 1
        }
    }

    Write-Host "Building plugin (this takes a few minutes the first time)..." -ForegroundColor Cyan
    # without rustup self-update – on Windows, it can lock files and crash the build
    $env:RUSTUP_AUTO_SELF_UPDATE = "disable"
    cargo build --release -p teren_plugin
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Retrying build..." -ForegroundColor Yellow
        Start-Sleep -Seconds 5
        cargo build --release -p teren_plugin
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: Build failed (see output above)." -ForegroundColor Red
        EndScript 1
    }
    $dll = "target\release\teren_plugin.dll"
}

Write-Host "Installing into Open CAD Studio..." -ForegroundColor Cyan
$target = Join-Path $env:APPDATA "OpenCADStudio\plugins\cz.fluvio.teren"
New-Item -ItemType Directory -Force -Path $target | Out-Null
Get-ChildItem $target -Filter *.dll -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue

try {
    Copy-Item $dll (Join-Path $target "teren_plugin.dll") -Force
} catch {
    Write-Host "ERROR: Cannot overwrite plugin – close Open CAD Studio and run the installation again." -ForegroundColor Red
    EndScript 1
}

$toml = Join-Path $PSScriptRoot "plugin.toml"
if (-not (Test-Path $toml)) {
    $toml = "plugin\plugin.toml"
}
Copy-Item $toml (Join-Path $target "plugin.toml") -Force

Write-Host ""
Write-Host "Done. The plugin is installed in:" -ForegroundColor Green
Write-Host "  $target"
Write-Host ""
Write-Host "Start (or restart) Open CAD Studio – the Terrain tab will appear."
EndScript 0
