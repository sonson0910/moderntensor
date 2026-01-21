<#
.SYNOPSIS
    Khởi động 3 nodes Luxtensor trên Windows

.DESCRIPTION
    Script này sẽ:
    1. Tạo thư mục node1, node2, node3
    2. Copy config files
    3. Khởi động 3 nodes trong các terminal riêng biệt

.EXAMPLE
    .\start-nodes-windows.ps1

.NOTES
    Chạy từ thư mục luxtensor
#>

param(
    [switch]$Clean,      # Xóa data cũ trước khi start
    [switch]$Build       # Build trước khi start
)

$ErrorActionPreference = "Stop"

# Colors
function Write-ColorOutput($ForegroundColor) {
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    if ($args) { Write-Output $args }
    $host.UI.RawUI.ForegroundColor = $fc
}

Write-Host ""
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Blue
Write-Host "  🚀 LuxTensor Multi-Node Startup Script (Windows)" -ForegroundColor Blue
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Blue
Write-Host ""

# Get script directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$LuxtensorRoot = Split-Path -Parent $ScriptDir

Set-Location $LuxtensorRoot
Write-Host "Working directory: $LuxtensorRoot" -ForegroundColor Cyan

# Build if requested
if ($Build) {
    Write-Host "`n📦 Building project..." -ForegroundColor Yellow
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Build failed!" -ForegroundColor Red
        exit 1
    }
    Write-Host "✅ Build successful!" -ForegroundColor Green
}

# Check if binary exists
$BinaryPath = Join-Path $LuxtensorRoot "target\release\luxtensor-node.exe"
if (-not (Test-Path $BinaryPath)) {
    Write-Host "❌ Error: luxtensor-node.exe not found!" -ForegroundColor Red
    Write-Host "Please build first: cargo build --release" -ForegroundColor Yellow
    exit 1
}

# Create node directories
Write-Host "`n📁 Creating node directories..." -ForegroundColor Yellow
foreach ($i in 1..3) {
    $NodeDir = Join-Path $LuxtensorRoot "node$i"
    if (-not (Test-Path $NodeDir)) {
        New-Item -ItemType Directory -Path $NodeDir -Force | Out-Null
        Write-Host "   Created node$i/" -ForegroundColor Green
    }
}

# Clean data if requested
if ($Clean) {
    Write-Host "`n🗑️  Cleaning old data..." -ForegroundColor Yellow
    foreach ($i in 1..3) {
        $DataDir = Join-Path $LuxtensorRoot "node$i\data"
        if (Test-Path $DataDir) {
            Remove-Item -Path $DataDir -Recurse -Force
            Write-Host "   Deleted node$i/data" -ForegroundColor Cyan
        }
    }
}

# Copy config files
Write-Host "`n📋 Copying config files..." -ForegroundColor Yellow
foreach ($i in 1..3) {
    $NodeDir = Join-Path $LuxtensorRoot "node$i"
    $ConfigDest = Join-Path $NodeDir "config.toml"
    $ConfigSrc = Join-Path $LuxtensorRoot "config.node$i.toml"

    if (Test-Path $ConfigSrc) {
        Copy-Item -Path $ConfigSrc -Destination $ConfigDest -Force
        Write-Host "   Copied config.node$i.toml → node$i/config.toml" -ForegroundColor Green
    }
    else {
        Write-Host "   ⚠️ config.node$i.toml not found!" -ForegroundColor Yellow
    }
}

# Start nodes in separate PowerShell windows
Write-Host "`n🚀 Starting nodes..." -ForegroundColor Yellow

$NodePorts = @{
    1 = @{ P2P = 30303; RPC = 8545 }
    2 = @{ P2P = 30304; RPC = 8555 }
    3 = @{ P2P = 30305; RPC = 8565 }
}

foreach ($i in 1..3) {
    $NodeDir = Join-Path $LuxtensorRoot "node$i"
    $P2P = $NodePorts[$i].P2P
    $RPC = $NodePorts[$i].RPC

    $Command = @"
Set-Location '$NodeDir'
Write-Host '🟢 Starting Node $i (P2P: $P2P, RPC: $RPC)' -ForegroundColor Green
Write-Host 'Press Ctrl+C to stop' -ForegroundColor Yellow
Write-Host ''
& '$BinaryPath' --config config.toml
"@

    Start-Process powershell -ArgumentList "-NoExit", "-Command", $Command
    Write-Host "   ✅ Node $i started (P2P: $P2P, RPC: $RPC)" -ForegroundColor Green

    # Wait a bit for node to start
    Start-Sleep -Seconds 2
}

Write-Host ""
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Green
Write-Host "  ✅ All 3 nodes started!" -ForegroundColor Green
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Green
Write-Host ""
Write-Host "Node Endpoints:" -ForegroundColor Cyan
Write-Host "  Node 1: http://localhost:8545" -ForegroundColor White
Write-Host "  Node 2: http://localhost:8555" -ForegroundColor White
Write-Host "  Node 3: http://localhost:8565" -ForegroundColor White
Write-Host ""
Write-Host "Các node sẽ tự động kết nối với nhau qua mDNS" -ForegroundColor Yellow
Write-Host "Kiểm tra trạng thái: .\scripts\check-nodes-windows.ps1" -ForegroundColor Yellow
Write-Host ""
