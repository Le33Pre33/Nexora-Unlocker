@echo off
setlocal EnableExtensions
chcp 65001 >nul
cd /d "%~dp0"

echo === Nexora Unlocker: сборка ===

where cargo >nul 2>nul
if errorlevel 1 goto :no_cargo

if /i "%~1"=="debug" goto :debug

cargo build --release
if errorlevel 1 goto :build_failed
set "EXE=%~dp0target\release\nexora-unlocker.exe"
goto :check_exe

:debug
cargo build
if errorlevel 1 goto :build_failed
set "EXE=%~dp0target\debug\nexora-unlocker.exe"

:check_exe
if not exist "%EXE%" goto :no_exe
echo [OK] Готово: "%EXE%"
exit /b 0

:no_cargo
echo [ОШИБКА] Не найден cargo - инструмент Rust. Запустите install-deps.bat.
exit /b 1

:build_failed
echo [ОШИБКА] Сборка не удалась. Если в тексте выше есть "link.exe not found" или "rc.exe",
echo          не хватает Visual Studio Build Tools - запустите install-deps.bat.
exit /b 1

:no_exe
echo [ОШИБКА] Сборка завершилась без ошибок, но файл не найден: "%EXE%"
exit /b 1
