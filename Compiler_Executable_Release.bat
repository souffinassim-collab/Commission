@echo off
title Compilation Release .EXE Portable
echo ========================================================
echo   COMPILATION DU BINAIRE .EXE PORTABLE FINAL
echo ========================================================
echo.
echo 1. Generation du logiciel autonome...
call npm run tauri build -- --no-bundle
echo.
echo 2. Creation du repertoire 'exe' et copie du fichier...
if not exist "exe" mkdir "exe"

if exist "src-tauri\target\release\comission-manager.exe" (
    copy /y "src-tauri\target\release\comission-manager.exe" "exe\Gestion_Commissions_Portable.exe"
    echo [SUCCES] L'executable 'Gestion_Commissions_Portable.exe' est pret dans le dossier 'exe' !
    echo Ouverture du dossier 'exe'...
    explorer "%~dp0exe"
) else (
    echo [ERREUR] Le binaire compile n'a pas ete trouve dans src-tauri\target\release.
)
pause
