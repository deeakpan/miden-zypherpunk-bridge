@echo off
REM Zcash Devtool Wallet CLI Helper - Batch Script
REM Usage: wallet-cli.bat [command] [wallet] [options]

setlocal enabledelayedexpansion

set COMMAND=%1
set WALLET=%2
if "%WALLET%"=="" set WALLET=bridge

set WALLET_DIR=%~dp0%WALLET%_wallet
set ZCASH_DEVTOOL_DIR=%~dp0zcash-devtool

if "%COMMAND%"=="balance" (
    echo Getting balance for %WALLET% wallet...
    cd /d "%ZCASH_DEVTOOL_DIR%"
    cargo run --release --all-features -- wallet -w "%WALLET_DIR%" balance
    goto :end
)

if "%COMMAND%"=="sync" (
    echo Syncing %WALLET% wallet...
    cd /d "%ZCASH_DEVTOOL_DIR%"
    cargo run --release --all-features -- wallet -w "%WALLET_DIR%" sync -s zecrocks
    goto :end
)

if "%COMMAND%"=="list-accounts" (
    echo Listing accounts for %WALLET% wallet...
    cd /d "%ZCASH_DEVTOOL_DIR%"
    cargo run --release --all-features -- wallet -w "%WALLET_DIR%" list-accounts
    goto :end
)

if "%COMMAND%"=="list-addresses" (
    echo Listing addresses for %WALLET% wallet...
    cd /d "%ZCASH_DEVTOOL_DIR%"
    cargo run --release --all-features -- wallet -w "%WALLET_DIR%" list-addresses
    goto :end
)

if "%COMMAND%"=="help" (
    goto :help
)

if "%COMMAND%"=="" (
    goto :help
)

:help
echo.
echo Zcash Devtool Wallet CLI Helper
echo.
echo Usage: wallet-cli.bat [command] [wallet]
echo.
echo Commands:
echo   balance          Get wallet balance
echo   sync             Sync wallet with blockchain
echo   list-accounts    List all accounts
echo   list-addresses   List addresses for account
echo   help             Show this help message
echo.
echo Wallet options:
echo   bridge           Use bridge_wallet (default)
echo   personal         Use personal_wallet
echo.
echo Examples:
echo   wallet-cli.bat balance
echo   wallet-cli.bat balance personal
echo   wallet-cli.bat sync
echo   wallet-cli.bat list-accounts
echo.
goto :end

:end
endlocal

