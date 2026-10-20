param([switch]$Test)
$ErrorActionPreference = 'Stop'
$workspaceRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$portable = Join-Path $workspaceRoot 'work\toolchain'
$portableCargo = Join-Path $portable 'cargo\bin\cargo.exe'
if (Test-Path -LiteralPath $portableCargo) {
    $env:CARGO_HOME = Join-Path $portable 'cargo'
    $env:RUSTUP_HOME = Join-Path $portable 'rustup'
    $env:PATH = "$(Join-Path $portable 'w64devkit\bin');$env:CARGO_HOME\bin;$env:PATH"
    $env:CARGO_TARGET_DIR = Join-Path $workspaceRoot 'work\target'
    $cargo = $portableCargo
    $toolchain = @('+stable-x86_64-pc-windows-gnu')
} else {
    $cargo = (Get-Command cargo -ErrorAction Stop).Source
    $toolchain = @()
}
$arguments = @('build', '--release', '--locked', '--manifest-path', (Join-Path $PSScriptRoot 'Cargo.toml'))
if ($Test) { $arguments[0] = 'test' }
& $cargo @toolchain @arguments
if ($LASTEXITCODE -ne 0) { throw "Cargo failed with exit code $LASTEXITCODE" }
if (!$Test) {
    $buildRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $PSScriptRoot 'target' }
    $version = [regex]::Match((Get-Content -LiteralPath (Join-Path $PSScriptRoot 'Cargo.toml') -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
    $outputDirectory = Split-Path $PSScriptRoot -Parent
    $binary = Join-Path $buildRoot 'release\media-shelf.exe'
    $versioned = Join-Path $outputDirectory "MediaShelf-v$version.exe"
    Copy-Item -LiteralPath $binary -Destination $versioned -Force
    try {
        Copy-Item -LiteralPath $binary -Destination (Join-Path $outputDirectory 'MediaShelf.exe') -Force -ErrorAction Stop
    } catch {
        Write-Warning "Could not replace MediaShelf.exe (it may be running). Updated build: $versioned"
    }
}
