# CONDUIT - build & play
#
# Uses the trusted MSVC toolchain targeting x86_64-pc-windows-gnu (with MinGW
# gcc on PATH) when available; falls back to standard cargo or Node/WASM if
# native execution is blocked by Smart App Control.

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$mingw = "$env:LOCALAPPDATA\mingw\mingw64\bin"
$cargoBin = "$env:USERPROFILE\.cargo\bin"
$msvcBin = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin"
$env:PATH = "$msvcBin;$mingw;$cargoBin;$env:PATH"

$msvcCargo = Join-Path $msvcBin "cargo.exe"
$msvcRustc = Join-Path $msvcBin "rustc.exe"

$exe = $null
if ((Test-Path $msvcCargo) -and (Test-Path $msvcRustc) -and (Test-Path "$mingw\gcc.exe")) {
    $env:RUSTC = $msvcRustc
    & $msvcCargo build --release --target x86_64-pc-windows-gnu
    if ($LASTEXITCODE -eq 0) {
        $exe = Join-Path $PSScriptRoot "target\x86_64-pc-windows-gnu\release\conduit.exe"
    }
}

if (-not $exe) {
    if (Test-Path "$mingw\gcc.exe") {
        cargo build --release
        if ($LASTEXITCODE -eq 0) {
            $exe = Join-Path $PSScriptRoot "target\release\conduit.exe"
        }
    }
}

if ($exe -and (Test-Path $exe)) {
    Write-Host ""
    Write-Host "Built $exe" -ForegroundColor Green
    Write-Host "Launching CONDUIT..." -ForegroundColor Green

    if ($args -contains "--test" -or $args -contains "--verify") {
        & $exe @args
        exit $LASTEXITCODE
    }

    try {
        & $exe @args
        exit $LASTEXITCODE
    } catch {
        Write-Host "Native execution failed; falling back to Node/WASM..." -ForegroundColor Yellow
    }
}

$node = Get-Command node -ErrorAction SilentlyContinue
if ($node) {
    & $node.Source (Join-Path $PSScriptRoot "tools\play-cli.mjs")
    exit $LASTEXITCODE
}

Write-Host "Neither native conduit.exe nor Node.js could be launched." -ForegroundColor Red
exit 1
