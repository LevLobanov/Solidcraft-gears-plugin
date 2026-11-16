$ErrorActionPreference = "Stop"

$PluginId = "custom-gears"
$CrateLib = "solidcraft_gear_plugin"
$DistDir  = Join-Path "dist" $PluginId
$DestDir  = Join-Path $env:LOCALAPPDATA "SolidCraft\plugins\$PluginId"

Write-Host "[1/4] Building wasm32-unknown-unknown..." -ForegroundColor Cyan
cargo build --target wasm32-unknown-unknown --release

Write-Host "[2/4] Embedding WIT & creating WebAssembly Component..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
wasm-tools component embed wit/ "target/wasm32-unknown-unknown/release/$CrateLib.wasm" -o target/embedded.wasm
wasm-tools component new target/embedded.wasm -o "$DistDir/plugin.wasm"
Copy-Item -Path "plugin.json" -Destination "$DistDir/plugin.json" -Force

Write-Host "[3/4] Packaging zip..." -ForegroundColor Cyan
Compress-Archive -Path $DistDir -DestinationPath "dist/$PluginId.zip" -Force

Write-Host "[4/4] Installing to $DestDir..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
Copy-Item -Path "$DistDir\*" -Destination $DestDir -Recurse -Force

Write-Host "Done! Plugin updated in $DestDir" -ForegroundColor Green