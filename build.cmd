@echo off
REM Build the Pantheon installer. Sets up the MSVC build environment first so the
REM Rust side always links, no matter which shell you start from. Double-click
REM this file, or run it from any terminal.
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
cd /d "%~dp0"
echo Building Pantheon (pnpm tauri build)...
pnpm tauri build
echo.
echo Done. Artifacts:
echo   src-tauri\target\release\pantheon.exe                  (standalone)
echo   src-tauri\target\release\bundle\nsis\*-setup.exe     (installer)
