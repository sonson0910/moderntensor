<#
.SYNOPSIS
    Kiểm tra trạng thái các nodes Luxtensor

.DESCRIPTION
    Script này kiểm tra:
    1. Nodes có đang chạy không
    2. Block height của mỗi node
    3. Số peers kết nối
    4. Sync status
#>

$ErrorActionPreference = "SilentlyContinue"

Write-Host ""
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Blue
Write-Host "  📊 LuxTensor Node Status Checker" -ForegroundColor Blue
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Blue
Write-Host ""

$Nodes = @(
    @{ Name = "Node 1"; Port = 8545 },
    @{ Name = "Node 2"; Port = 8555 },
    @{ Name = "Node 3"; Port = 8565 }
)

$BlockHeights = @()

foreach ($Node in $Nodes) {
    $Name = $Node.Name
    $Port = $Node.Port
    $Url = "http://localhost:$Port"

    Write-Host "─────────────────────────────────────" -ForegroundColor Gray
    Write-Host "📡 $Name (Port: $Port)" -ForegroundColor Cyan

    try {
        # Check block number
        $BlockRequest = @{
            jsonrpc = "2.0"
            method  = "eth_blockNumber"
            params  = @()
            id      = 1
        } | ConvertTo-Json

        $BlockResponse = Invoke-RestMethod -Uri $Url -Method POST -Body $BlockRequest -ContentType "application/json" -TimeoutSec 5
        $BlockHex = $BlockResponse.result
        $BlockNumber = [Convert]::ToInt64($BlockHex, 16)
        $BlockHeights += $BlockNumber

        Write-Host "   ✅ Online" -ForegroundColor Green
        Write-Host "   📦 Block Height: $BlockNumber" -ForegroundColor White

        # Check peer count
        $PeerRequest = @{
            jsonrpc = "2.0"
            method  = "net_peerCount"
            params  = @()
            id      = 2
        } | ConvertTo-Json

        $PeerResponse = Invoke-RestMethod -Uri $Url -Method POST -Body $PeerRequest -ContentType "application/json" -TimeoutSec 5
        if ($PeerResponse.result) {
            $PeerHex = $PeerResponse.result
            $PeerCount = [Convert]::ToInt64($PeerHex, 16)
            Write-Host "   👥 Connected Peers: $PeerCount" -ForegroundColor White
        }

        # Check node info (chain_id, syncing)
        $ChainRequest = @{
            jsonrpc = "2.0"
            method  = "eth_chainId"
            params  = @()
            id      = 3
        } | ConvertTo-Json

        $ChainResponse = Invoke-RestMethod -Uri $Url -Method POST -Body $ChainRequest -ContentType "application/json" -TimeoutSec 5
        if ($ChainResponse.result) {
            $ChainId = [Convert]::ToInt64($ChainResponse.result, 16)
            Write-Host "   ⛓️  Chain ID: $ChainId" -ForegroundColor White
        }

    }
    catch {
        Write-Host "   ❌ Offline or not responding" -ForegroundColor Red
        Write-Host "   Error: $($_.Exception.Message)" -ForegroundColor DarkGray
    }
}

Write-Host ""
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Blue
Write-Host "  📈 Sync Summary" -ForegroundColor Blue
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Blue

if ($BlockHeights.Count -gt 0) {
    $MaxHeight = ($BlockHeights | Measure-Object -Maximum).Maximum
    $MinHeight = ($BlockHeights | Measure-Object -Minimum).Minimum
    $AvgHeight = [math]::Round(($BlockHeights | Measure-Object -Average).Average)

    Write-Host ""
    Write-Host "   Highest Block: $MaxHeight" -ForegroundColor Green
    Write-Host "   Lowest Block:  $MinHeight" -ForegroundColor Yellow
    Write-Host "   Average Block: $AvgHeight" -ForegroundColor White

    if ($MaxHeight -eq $MinHeight) {
        Write-Host ""
        Write-Host "   ✅ All nodes are in sync!" -ForegroundColor Green
    }
    else {
        $Diff = $MaxHeight - $MinHeight
        Write-Host ""
        Write-Host "   ⚠️  Nodes are $Diff blocks apart" -ForegroundColor Yellow
        Write-Host "   Sync in progress..." -ForegroundColor Yellow
    }
}
else {
    Write-Host "   ❌ No nodes are responding" -ForegroundColor Red
}

Write-Host ""
