@echo off
setlocal EnableExtensions
chcp 65001 >nul
cd /d "%~dp0"

rem Использование: package.bat [--portable-only]
rem Результат в папке dist: Portable-ZIP и (без флага) Setup.exe

set "PORTABLE_ONLY=0"
if /i "%~1"=="--portable-only" set "PORTABLE_ONLY=1"

echo === Nexora Unlocker: упаковка ===

call "%~dp0build.bat"
if errorlevel 1 goto :fail_build

rem ---------- версия из Cargo.toml (первая строка version = "...") ----------
set "VERSION="
for /f "usebackq tokens=2 delims==" %%v in (`findstr /b /c:"version = " "%~dp0Cargo.toml"`) do if not defined VERSION set VERSION=%%v
if not defined VERSION goto :fail_version
set VERSION=%VERSION:"=%
set VERSION=%VERSION: =%
echo Версия: %VERSION%

rem ---------- чистая папка dist ----------
set "DIST=%~dp0dist"
if exist "%DIST%" rmdir /s /q "%DIST%"
mkdir "%DIST%"
if errorlevel 1 goto :fail_dist

rem ---------- Portable ----------
set "PORT=%DIST%\NexoraUnlocker-Portable"
mkdir "%PORT%"
copy /y "%~dp0target\release\nexora-unlocker.exe" "%PORT%\" >nul
if errorlevel 1 goto :fail_copy
copy /y "%~dp0README.md" "%PORT%\" >nul
rem portable.flag: настройки хранятся рядом с программой, а не в %%APPDATA%%
type nul > "%PORT%\portable.flag"
powershell -NoProfile -ExecutionPolicy Bypass -Command "Compress-Archive -LiteralPath '%PORT%' -DestinationPath '%DIST%\NexoraUnlocker-Portable-%VERSION%.zip' -Force"
if errorlevel 1 goto :fail_zip

if "%PORTABLE_ONLY%"=="1" goto :done

rem ---------- Установщик (Inno Setup) ----------
set "ISCC="
if exist "%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe" set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if not defined ISCC if exist "%ProgramFiles%\Inno Setup 6\ISCC.exe" set "ISCC=%ProgramFiles%\Inno Setup 6\ISCC.exe"
if not defined ISCC if exist "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" set "ISCC=%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe"
if not defined ISCC goto :fail_iscc

"%ISCC%" /Qp "/DAppVersion=%VERSION%" "/O%DIST%" "%~dp0installer\nexora.iss"
if errorlevel 1 goto :fail_iscc_run

:done
echo.
echo [OK] Готово. Файлы в папке: "%DIST%"
dir /b "%DIST%"
exit /b 0

:fail_build
echo [ОШИБКА] Сборка не удалась, упаковка прервана.
exit /b 1
:fail_version
echo [ОШИБКА] Не удалось прочитать версию из Cargo.toml.
exit /b 1
:fail_dist
echo [ОШИБКА] Не удалось создать папку "%DIST%".
exit /b 1
:fail_copy
echo [ОШИБКА] Не удалось скопировать nexora-unlocker.exe.
exit /b 1
:fail_zip
echo [ОШИБКА] Не удалось создать ZIP-архив.
exit /b 1
:fail_iscc
echo [ОШИБКА] Не найден Inno Setup. Запустите install-deps.bat --installer
echo          или соберите только переносную версию: package.bat --portable-only
exit /b 1
:fail_iscc_run
echo [ОШИБКА] Inno Setup не смог создать установщик (текст ошибки выше).
exit /b 1
