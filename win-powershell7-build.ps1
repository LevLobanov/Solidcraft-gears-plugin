$ErrorActionPreference = "Stop"

$PluginId = "custom-gears"
$CrateLib = "solidcraft_gear_plugin"
$DistDir  = Join-Path "dist" $PluginId
$ZipPath  = Join-Path "dist" "$PluginId.zip"

Write-Host "[1/4] Running unit & mock tests..." -ForegroundColor Cyan
cargo test

Write-Host "[2/4] Building wasm32-unknown-unknown..." -ForegroundColor Cyan
cargo build --target wasm32-unknown-unknown --release

Write-Host "[3/4] Embedding WIT & packaging Component + icons..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path "$DistDir\icons" | Out-Null
wasm-tools component embed wit/ "target/wasm32-unknown-unknown/release/$CrateLib.wasm" -o target/embedded.wasm
wasm-tools component new target/embedded.wasm -o "$DistDir/plugin.wasm"
Copy-Item -Path "plugin.json" -Destination "$DistDir/plugin.json" -Force
Copy-Item -Path "icons\gear.svg" -Destination "$DistDir\icons\gear.svg" -Force
Compress-Archive -Path $DistDir -DestinationPath $ZipPath -Force

Write-Host "[4/4] Checking with solidcraft-cli (if on PATH)..." -ForegroundColor Cyan
if (Get-Command solidcraft-cli -ErrorAction SilentlyContinue) {
    solidcraft-cli plugin check $ZipPath
}

Write-Host "Done! Install $ZipPath via Preferences -> Plugins -> Install plugin..." -ForegroundColor Green