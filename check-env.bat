@echo off
CALL "D:\VSCoding 2022\VC\Auxiliary\Build\vcvarsall.bat" x64
echo === LIB ===
echo %LIB%
echo === INCLUDE ===
echo %INCLUDE%
echo === WindowsSdkDir ===
echo %WindowsSdkDir%
echo === UCRTVersion ===
echo %UCRTVersion%
dir /s /b %WindowsSdkDir%\Lib\*kernel32.lib 2>nul
