#!/bin/bash

# Run relayers separately for easier debugging
# Open 3 separate terminals:
# 1. Backend API
# 2. Zcash Relayer (Zcash → Miden)
# 3. Miden Exit Relayer (Miden → Zcash)

echo "=== Miden-Zcash Bridge Relayers ==="
echo ""
echo "Run these commands in separate terminals:"
echo ""
echo "Terminal 1 - Backend API:"
echo "  cd rust-backend"
echo "  cargo run --release"
echo ""
echo "Terminal 2 - Zcash Relayer (Zcash → Miden):"
echo "  cd rust-backend"
echo "  export ZCASH_RELAYER_INTERVAL_SECS=5"
echo "  cargo run --release --bin zcash_relayer"
echo ""
echo "Terminal 3 - Miden Exit Relayer (Miden → Zcash):"
echo "  cd rust-backend"
echo "  export MIDEN_RELAYER_INTERVAL_SECS=10"
echo "  cargo run --release --bin miden_exit_relayer"
echo ""
