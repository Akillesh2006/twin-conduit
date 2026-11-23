# CONDUIT - one-time toolchain setup (portable MinGW-w64, no admin needed)
#
# The game itself has zero dependencies, so all we need is a linker for the
# Windows GNU target. This machine's newer rustc binaries are blocked by Smart
# App Control, but 1.85.0 works, so that is the toolchain run.ps1 prefers.
$ErrorActionPreference = "Stop"
$dest = "$env:LOCALAPPDATA\mingw"
$zip  = "$env:TEMP\winlibs.zip"

if (-not (Test-Path "$dest\mingw64\bin\gcc.exe")) {
    if (-not (Test-Path $zip)) {
        Write-Host "Downloading MinGW-w64 (~260 MB)..."
        Invoke-WebRequest -UseBasicParsing -OutFile $zip `
            "https://github.com/brechtsanders/winlibs_mingw/releases/download/16.2.0posix-23.1.3-14.0.0-ucrt-r3/winlibs-x86_64-posix-seh-gcc-16.2.0-mingw-w64ucrt-14.0.0-r3.zip"
    }
    Write-Host "Extracting..."
    Remove-Item $dest -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Path $dest -Force | Out-Null
    Expand-Archive -Path $zip -DestinationPath $dest -Force
}

$env:PATH = "$dest\mingw64\bin;$env:USERPROFILE\.cargo\bin;$env:PATH"

# 1.85.0 is the last toolchain this machine's Smart App Control allows to run.
rustup toolchain install 1.85.0-x86_64-pc-windows-gnu --profile minimal
rustup target add wasm32-unknown-unknown --toolchain 1.85.0-x86_64-pc-windows-gnu

gcc --version | Select-Object -First 1
Write-Host "Done. Run .\run.ps1" -ForegroundColor Green
