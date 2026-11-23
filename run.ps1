# OVERDRIVE - build & play
#
# On this machine the GNU toolchain's own cargo.exe is blocked by Smart App
# Control, so we let the trusted MSVC cargo.exe drive the 1.85.0 GNU rustc.
# If your machine is not restricted, plain `cargo run --release` works fine.

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$mingw = "$env:LOCALAPPDATA\mingw\mingw64\bin"
$cargoBin = "$env:USERPROFILE\.cargo\bin"
$env:PATH = "$mingw;$cargoBin;$env:PATH"

if (-not (Test-Path "$mingw\gcc.exe")) {
    Write-Host "MinGW toolchain missing at $mingw" -ForegroundColor Red
    Write-Host "Run setup.ps1 first."
    exit 1
}

$gnu = "$env:USERPROFILE\.rustup\toolchains\1.85.0-x86_64-pc-windows-gnu\bin\rustc.exe"
$msvcCargo = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe"

if (Test-Path $gnu) {
    $env:RUSTC = $gnu
}
if (Test-Path $msvcCargo) {
    & $msvcCargo build --release
} else {
    cargo build --release
}

if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = Join-Path $PSScriptRoot "target\release\overdrive.exe"
Write-Host ""
Write-Host "Built $exe" -ForegroundColor Green
Write-Host "Launching..." -ForegroundColor Green

if ($args -contains "--test" -or $args -contains "--verify") {
    & $exe @args
    exit $LASTEXITCODE
}

try {
    & $exe @args
    exit $LASTEXITCODE
} catch {
    $node = Get-Command node -ErrorAction SilentlyContinue
    if ($node) {
        & $node.Source (Join-Path $PSScriptRoot "tools\play-cli.mjs")
        exit $LASTEXITCODE
    }
    throw $_
}
