# Wallet CLI Helper Scripts

Helper scripts to simplify running common zcash-devtool wallet commands.

## Quick Start

### PowerShell (Recommended)

```powershell
# Get balance
.\wallet-cli.ps1 balance

# Sync wallet
.\wallet-cli.ps1 sync

# List accounts
.\wallet-cli.ps1 list-accounts

# List addresses
.\wallet-cli.ps1 list-addresses

# Use personal wallet instead of bridge wallet
.\wallet-cli.ps1 balance -Wallet personal
```

### Batch File (Simple)

```batch
# Get balance
wallet-cli.bat balance

# Sync wallet
wallet-cli.bat sync

# Use personal wallet
wallet-cli.bat balance personal
```

## Available Commands

### Basic Commands

- `balance` - Get wallet balance
- `sync` - Sync wallet with blockchain
- `list-accounts` - List all accounts in the wallet
- `list-addresses` - List addresses for an account
- `list-tx` - List transactions
- `list-unspent` - List unspent notes

### Advanced Commands (PowerShell only)

- `gen-addr` - Generate a new address (requires `-AccountId`)
- `send` - Send funds (requires `-Address` and `-Amount`)

## PowerShell Script Options

```powershell
# Full syntax
.\wallet-cli.ps1 [command] [options]

# Options:
-Wallet <bridge|personal>    # Select wallet (default: bridge)
-AccountId <uuid>            # Account UUID (for some commands)
-Address <address>           # Recipient address (for send)
-Amount <amount>             # Amount in zatoshis (for send)
-Memo <memo>                 # Memo text (for send, optional)
-Identity <path>             # Path to identity file (default: wallet/key.txt)
-UseBuilt                    # Use built executable instead of cargo run
```

## Examples

### Check Balance
```powershell
.\wallet-cli.ps1 balance
.\wallet-cli.ps1 balance -Wallet personal
```

### Sync Wallet
```powershell
.\wallet-cli.ps1 sync
```

### List Accounts and Addresses
```powershell
# List all accounts
.\wallet-cli.ps1 list-accounts

# List addresses for a specific account
.\wallet-cli.ps1 list-addresses -AccountId <account-uuid>
```

### Generate New Address
```powershell
.\wallet-cli.ps1 gen-addr -AccountId <account-uuid>
```

### Send Funds
```powershell
.\wallet-cli.ps1 send -Address <recipient-address> -Amount 1000000 -Memo "Test payment"
```

## Direct CLI Usage

If you need more control, you can use the zcash-devtool CLI directly:

```powershell
cd wallet\zcash-devtool
cargo run --release --all-features -- wallet -w ..\bridge_wallet balance
cargo run --release --all-features -- wallet -w ..\bridge_wallet sync -s zecrocks
```

## Building the Executable

To build the executable for faster execution:

```powershell
cd wallet\zcash-devtool
cargo build --release --all-features
```

Then use the `-UseBuilt` flag:

```powershell
.\wallet-cli.ps1 balance -UseBuilt
```

## Wallet Directories

- `bridge_wallet/` - Bridge wallet data
- `personal_wallet/` - Personal wallet data

Each wallet directory contains:
- `keys.toml` - Encrypted wallet keys
- `key.txt` - Age identity file for decryption
- `data.sqlite` - Wallet database
- `blockmeta.sqlite` - Block metadata

## Notes

- The scripts automatically use the correct wallet directory based on the `-Wallet` parameter
- Identity files default to `key.txt` in the wallet directory
- All commands use the testnet by default (`-s zecrocks`)
- Make sure you have Rust and Cargo installed to build/run the tool

