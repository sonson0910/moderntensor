# PowerShell script to start 3 LuxTensor nodes on Windows
# Using debug binary (has lux_registerMiner fix)

$BinaryPath = "$PSScriptRoot\target\debug\luxtensor-node.exe"
$LuxDir = $PSScriptRoot

if (-not (Test-Path $BinaryPath)) {
    Write-Host "Binary not found at: $BinaryPath" -ForegroundColor Red
    exit 1
}

Write-Host "Starting 3 LuxTensor nodes..." -ForegroundColor Green

# Create node data directories
foreach ($i in 1,2,3) {
    New-Item -ItemType Directory -Force -Path "$LuxDir\data_node$i" | Out-Null
    New-Item -ItemType Directory -Force -Path "$LuxDir\data_node$i\db" | Out-Null
}

# Start Node 1 (port 8545)
$p1 = Start-Process -FilePath $BinaryPath `
    -ArgumentList "--config", "$LuxDir\config.node1.toml" `
    -WorkingDirectory $LuxDir `
    -PassThru -WindowStyle Minimized
Write-Host "Node 1 started (PID=$($p1.Id), RPC=http://localhost:8545)" -ForegroundColor Cyan

Start-Sleep -Seconds 2

# Start Node 2 (port 8555)
$p2 = Start-Process -FilePath $BinaryPath `
    -ArgumentList "--config", "$LuxDir\config.node2.toml" `
    -WorkingDirectory $LuxDir `
    -PassThru -WindowStyle Minimized
Write-Host "Node 2 started (PID=$($p2.Id), RPC=http://localhost:8555)" -ForegroundColor Cyan

Start-Sleep -Seconds 2

# Start Node 3 (port 8565)
$p3 = Start-Process -FilePath $BinaryPath `
    -ArgumentList "--config", "$LuxDir\config.node3.toml" `
    -WorkingDirectory $LuxDir `
    -PassThru -WindowStyle Minimized
Write-Host "Node 3 started (PID=$($p3.Id), RPC=http://localhost:8565)" -ForegroundColor Cyan

Write-Host ""
Write-Host "All nodes started! Waiting 5 seconds for them to initialize..." -ForegroundColor Green
Start-Sleep -Seconds 5

# Quick health check
Write-Host "Checking node health..." -ForegroundColor Yellow
foreach ($port in 8545, 8555, 8565) {
    try {
        $body = '{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}'
        $resp = Invoke-RestMethod -Uri "http://localhost:$port" `
            -Method Post -Body $body -ContentType "application/json" -TimeoutSec 3
        Write-Host "  Node :$port -> Block: $($resp.result)" -ForegroundColor Green
    } catch {
        Write-Host "  Node :$port -> Not ready yet" -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "PIDs: $($p1.Id), $($p2.Id), $($p3.Id)" -ForegroundColor Gray
Write-Host "To stop: Stop-Process -Id $($p1.Id),$($p2.Id),$($p3.Id)" -ForegroundColor Gray
