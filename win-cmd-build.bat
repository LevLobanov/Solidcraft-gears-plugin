@echo off
setlocal enabledelayedexpansion

set "PLUGIN_ID=custom-gears"
set "CRATE_LIB=solidcraft_gear_plugin"
set "DIST_DIR=dist\%PLUGIN_ID%"
set "DEST_DIR=%LOCALAPPDATA%\SolidCraft\plugins\%PLUGIN_ID%"

echo [1/4] Building wasm32-unknown-unknown...
cargo build --target wasm32-unknown-unknown --release
if errorlevel 1 exit /b 1

echo [2/4] Embedding WIT and creating WebAssembly Component...
if not exist "%DIST_DIR%" mkdir "%DIST_DIR%"
wasm-tools component embed wit\ "target\wasm32-unknown-unknown\release\%CRATE_LIB%.wasm" -o target\embedded.wasm
if errorlevel 1 exit /b 1

wasm-tools component new target\embedded.wasm -o "%DIST_DIR%\plugin.wasm"
if errorlevel 1 exit /b 1

copy /Y plugin.json "%DIST_DIR%\plugin.json" >nul

echo [3/4] Packaging zip...
powershell -NoProfile -Command "Compress-Archive -Path '%DIST_DIR%' -DestinationPath 'dist\%PLUGIN_ID%.zip' -Force"
if errorlevel 1 exit /b 1

echo [4/4] Installing to %DEST_DIR%...
if not exist "%DEST_DIR%" mkdir "%DEST_DIR%"
xcopy /E /Y /Q "%DIST_DIR%\*" "%DEST_DIR%\" >nul

echo Done! Installed to %DEST_DIR%
endlocal