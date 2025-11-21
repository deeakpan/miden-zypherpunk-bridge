# Zcash Devtool Wallet CLI Helper Script
# Usage: .\wallet-cli.ps1 [command] [options]

param(
    [Parameter(Position=0)]
    [ValidateSet("balance", "sync", "list-addresses", "list-accounts", "list-tx", "list-unspent", "send", "gen-addr", "help")]
    [string]$Command = "help",
    
    [Parameter()]
    [ValidateSet("bridge", "personal")]
    [string]$Wallet = "bridge",
    
    [Parameter()]
    [string]$AccountId = "",
    
    [Parameter()]
    [string]$Address = "",
    
    [Parameter()]
    [string]$Amount = "",
    
    [Parameter()]
    [string]$Memo = "",
    
    [Parameter()]
    [string]$Identity = "",
    
    [Parameter()]
    [switch]$UseBuilt = $false
)

# Set wallet directory
$WalletDir = if ($Wallet -eq "bridge") { 
    "$PSScriptRoot\bridge_wallet" 
} else { 
    "$PSScriptRoot\personal_wallet" 
}

# Set identity file path (default to key.txt in wallet directory)
$IdentityFile = if ($Identity) { 
    $Identity 
} else { 
    "$WalletDir\key.txt" 
}

# Determine if we should use built executable or cargo run
$ZcashDevtoolPath = "$PSScriptRoot\zcash-devtool\target\release\zcash-devtool.exe"
$UseCargo = -not ($UseBuilt -and (Test-Path $ZcashDevtoolPath))

# Base command
function Invoke-ZcashCommand {
    param([string[]]$Args)
    
    if ($UseCargo) {
        $cargoArgs = @("run", "--release", "--all-features", "--") + $Args
        Push-Location "$PSScriptRoot\zcash-devtool"
        try {
            cargo $cargoArgs
        } finally {
            Pop-Location
        }
    } else {
        & $ZcashDevtoolPath $Args
    }
}

# Commands
switch ($Command) {
    "balance" {
        Write-Host "Getting balance for $Wallet wallet..." -ForegroundColor Cyan
        Invoke-ZcashCommand @("wallet", "-w", $WalletDir, "balance")
    }
    
    "sync" {
        Write-Host "Syncing $wallet wallet..." -ForegroundColor Cyan
        Invoke-ZcashCommand @("wallet", "-w", $WalletDir, "sync", "-s", "zecrocks")
    }
    
    "list-addresses" {
        Write-Host "Listing addresses for $wallet wallet..." -ForegroundColor Cyan
        $args = @("wallet", "-w", $WalletDir, "list-addresses")
        if ($AccountId) {
            $args += "--account-id", $AccountId
        }
        Invoke-ZcashCommand $args
    }
    
    "list-accounts" {
        Write-Host "Listing accounts for $wallet wallet..." -ForegroundColor Cyan
        Invoke-ZcashCommand @("wallet", "-w", $WalletDir, "list-accounts")
    }
    
    "list-tx" {
        Write-Host "Listing transactions for $wallet wallet..." -ForegroundColor Cyan
        $args = @("wallet", "-w", $WalletDir, "list-tx")
        if ($AccountId) {
            $args += "--account-id", $AccountId
        }
        Invoke-ZcashCommand $args
    }
    
    "list-unspent" {
        Write-Host "Listing unspent notes for $wallet wallet..." -ForegroundColor Cyan
        $args = @("wallet", "-w", $WalletDir, "list-unspent")
        if ($AccountId) {
            $args += "--account-id", $AccountId
        }
        Invoke-ZcashCommand $args
    }
    
    "gen-addr" {
        Write-Host "Generating new address for $wallet wallet..." -ForegroundColor Cyan
        if (-not $AccountId) {
            Write-Host "Error: --AccountId is required for gen-addr" -ForegroundColor Red
            exit 1
        }
        Invoke-ZcashCommand @("wallet", "-w", $WalletDir, "gen-addr", "--account-id", $AccountId)
    }
    
    "send" {
        Write-Host "Sending funds from $wallet wallet..." -ForegroundColor Cyan
        if (-not $Address) {
            Write-Host "Error: --Address is required for send" -ForegroundColor Red
            exit 1
        }
        if (-not $Amount) {
            Write-Host "Error: --Amount is required for send" -ForegroundColor Red
            exit 1
        }
        if (-not (Test-Path $IdentityFile)) {
            Write-Host "Error: Identity file not found at $IdentityFile" -ForegroundColor Red
            exit 1
        }
        
        $args = @("wallet", "-w", $WalletDir, "send", "-i", $IdentityFile, "--address", $Address, "--value", $Amount)
        if ($AccountId) {
            $args += "--account-id", $AccountId
        }
        if ($Memo) {
            $args += "--memo", $Memo
        }
        Invoke-ZcashCommand $args
    }
    
    "help" {
        Write-Host @"
Zcash Devtool Wallet CLI Helper

Usage:
    .\wallet-cli.ps1 [command] [options]

Commands:
    balance          Get wallet balance
    sync             Sync wallet with blockchain
    list-addresses   List addresses for account
    list-accounts    List all accounts
    list-tx          List transactions
    list-unspent     List unspent notes
    gen-addr         Generate new address (requires --AccountId)
    send             Send funds (requires --Address and --Amount)
    help             Show this help message

Options:
    -Wallet <bridge|personal>    Select wallet (default: bridge)
    -AccountId <uuid>            Account UUID (for some commands)
    -Address <address>           Recipient address (for send)
    -Amount <amount>             Amount in zatoshis (for send)
    -Memo <memo>                 Memo text (for send, optional)
    -Identity <path>             Path to identity file (default: wallet/key.txt)
    -UseBuilt                    Use built executable instead of cargo run

Examples:
    .\wallet-cli.ps1 balance
    .\wallet-cli.ps1 balance -Wallet personal
    .\wallet-cli.ps1 sync
    .\wallet-cli.ps1 list-accounts
    .\wallet-cli.ps1 list-addresses -AccountId <uuid>
    .\wallet-cli.ps1 gen-addr -AccountId <uuid>
    .\wallet-cli.ps1 send -Address <addr> -Amount 1000000 -Memo "Test payment"
"@ -ForegroundColor Green
    }
}

