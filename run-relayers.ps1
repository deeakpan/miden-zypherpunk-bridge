# Run relayers separately for easier debugging
# Open 3 separate terminals:
# 1. Backend API
# 2. Zcash Relayer (Zcash → Miden)
# 3. Miden Exit Relayer (Miden → Zcash)

Write-Host "=== Miden-Zcash Bridge Relayers ===" -ForegroundColor Green
Write-Host ""
Write-Host "Run these commands in separate terminals:" -ForegroundColor Yellow
Write-Host ""
Write-Host "Terminal 1 - Backend API:" -ForegroundColor Cyan
Write-Host "  cd rust-backend"
Write-Host "  cargo run --release"
Write-Host ""
Write-Host "Terminal 2 - Zcash Relayer (Zcash → Miden):" -ForegroundColor Cyan
Write-Host "  cd rust-backend"
Write-Host "  `$env:ZCASH_RELAYER_INTERVAL_SECS = '5'"
Write-Host "  cargo run --release --bin zcash_relayer"
Write-Host ""
Write-Host "Terminal 3 - Miden Exit Relayer (Miden → Zcash):" -ForegroundColor Cyan
Write-Host "  cd rust-backend"
Write-Host "  `$env:MIDEN_RELAYER_INTERVAL_SECS = '10'"
Write-Host "  cargo run --release --bin miden_exit_relayer"
Write-Host ""
