@echo off
setlocal EnableExtensions
chcp 65001 >nul
cd /d "%~dp0"

rem Использование: install-deps.bat [--yes] [--installer]
rem   --yes        не задавать вопросов
rem   --installer  дополнительно проверить Inno Setup (нужен для Setup.exe)

set "ASSUME_YES=0"
set "WITH_INSTALLER=0"
:parse
if "%~1"=="" goto :parsed
if /i "%~1"=="--yes" set "ASSUME_YES=1"
if /i "%~1"=="--installer" set "WITH_INSTALLER=1"
shift
goto :parse
:parsed

echo === Nexora Unlocker: проверка инструментов разработчика ===
set "FAILED=0"
set "RESTART=0"

set "HAVE_WINGET=0"
where winget >nul 2>nul
if not errorlevel 1 set "HAVE_WINGET=1"

rem ---------- Rust ----------
set "NEED_RUST=0"
where cargo >nul 2>nul
if errorlevel 1 set "NEED_RUST=1"
if "%NEED_RUST%"=="0" echo [OK]  Rust найден
if "%NEED_RUST%"=="1" echo [НЕТ] Rust не найден

rem ---------- Visual Studio Build Tools (компилятор C++ и Windows SDK) ----------
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "NEED_MSVC=1"
if not exist "%VSWHERE%" goto :msvc_done
for /f "usebackq delims=" %%i in (`""%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath"`) do set "NEED_MSVC=0"
:msvc_done
if "%NEED_MSVC%"=="0" echo [OK]  Visual Studio Build Tools найдены
if "%NEED_MSVC%"=="1" echo [НЕТ] Visual Studio Build Tools не найдены

rem ---------- Inno Setup (только для установщика) ----------
set "NEED_INNO=0"
if "%WITH_INSTALLER%"=="0" goto :inno_checked
set "ISCC="
if exist "%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe" set "ISCC=1"
if exist "%ProgramFiles%\Inno Setup 6\ISCC.exe" set "ISCC=1"
if exist "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" set "ISCC=1"
if not defined ISCC set "NEED_INNO=1"
if "%NEED_INNO%"=="0" echo [OK]  Inno Setup найден
if "%NEED_INNO%"=="1" echo [НЕТ] Inno Setup не найден
:inno_checked

if "%NEED_RUST%%NEED_MSVC%%NEED_INNO%"=="000" goto :all_ok

if "%HAVE_WINGET%"=="1" goto :install
echo.
echo [ОШИБКА] Не найден winget, автоустановка невозможна. Установите вручную:
if "%NEED_RUST%"=="1"  echo   Rust:                  https://rustup.rs
if "%NEED_MSVC%"=="1"  echo   Visual Studio Build Tools: https://visualstudio.microsoft.com/visual-cpp-build-tools/
if "%NEED_INNO%"=="1"  echo   Inno Setup:            https://jrsoftware.org/isdl.php
exit /b 1

:install
echo.
if "%NEED_MSVC%"=="0" goto :inst_rust
call :confirm "Установить Visual Studio Build Tools (около 3-6 ГБ)"
if errorlevel 1 goto :skip_msvc
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-package-agreements --accept-source-agreements --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
if errorlevel 1 set "FAILED=1"
set "RESTART=1"
goto :inst_rust
:skip_msvc
set "FAILED=1"

:inst_rust
if "%NEED_RUST%"=="0" goto :inst_inno
call :confirm "Установить Rust (rustup)"
if errorlevel 1 goto :skip_rust
winget install --id Rustlang.Rustup -e --accept-package-agreements --accept-source-agreements
if errorlevel 1 set "FAILED=1"
set "RESTART=1"
goto :inst_inno
:skip_rust
set "FAILED=1"

:inst_inno
if "%NEED_INNO%"=="0" goto :finish
call :confirm "Установить Inno Setup"
if errorlevel 1 goto :skip_inno
winget install --id JRSoftware.InnoSetup -e --accept-package-agreements --accept-source-agreements
if errorlevel 1 set "FAILED=1"
goto :finish
:skip_inno
set "FAILED=1"

:finish
if "%FAILED%"=="1" goto :failed
if "%RESTART%"=="1" echo.
if "%RESTART%"=="1" echo Установка завершена. Закройте это окно, откройте новое и запустите install-deps.bat ещё раз для проверки.
exit /b 0

:all_ok
echo.
echo Все нужные инструменты установлены. Можно запускать build.bat.
exit /b 0

:failed
echo.
echo [ОШИБКА] Не все инструменты установлены. Повторите запуск или установите их вручную.
exit /b 1

:confirm
if "%ASSUME_YES%"=="1" exit /b 0
choice /c YN /n /m "%~1? [Y/N] "
if errorlevel 2 exit /b 1
exit /b 0
