@echo off
rem Runs a command with the Visual Studio Build Tools 2022 C++ environment loaded, when they are installed.
rem Some machines also have a Visual Studio preview whose linker lacks the C++ libraries Rust needs
rem (LNK1104: msvcrt.lib). Loading the Build Tools first avoids that. Without them, the command runs as is.
set "TERN_VS="
for /f "usebackq tokens=*" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -products Microsoft.VisualStudio.Product.BuildTools -latest -property installationPath 2^>nul`) do set "TERN_VS=%%i"
if defined TERN_VS (
  set "PATH=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer;%PATH%"
  call "%TERN_VS%\VC\Auxiliary\Build\vcvars64.bat" >nul
)
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
%*
