# LuxTensor Test Runner Script
# Comprehensive test execution with multiple modes

param (
    [string]$Mode = "all",  # all, unit, rpc, network, bench
    [switch]$Clean = $false,
    [switch]$StartNodes = $false,
    [switch]$Verbose = $false
)

$ErrorActionPreference = "Stop"
$LuxtensorDir = Split-Path -Parent $PSScriptRoot

Write-Host "╔═══════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║          LuxTensor Test Runner                ║" -ForegroundColor Cyan
Write-Host "╚═══════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Change to luxtensor directory
Set-Location $LuxtensorDir

# Clean if requested
if ($Clean) {
    Write-Host "🧹 Cleaning previous test data..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force node1\data, node2\data, node3\data -ErrorAction SilentlyContinue
    cargo clean --package luxtensor-tests 2>&1 | Out-Null
}

# Build tests first
Write-Host "🔨 Building tests..." -ForegroundColor Yellow
cargo build --package luxtensor-tests --all-targets
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Build successful" -ForegroundColor Green
Write-Host ""

# Start nodes if requested
if ($StartNodes) {
    Write-Host "🚀 Starting nodes..." -ForegroundColor Yellow

    # Kill existing nodes
    Get-Process luxtensor-node -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Seconds 2

    # Clean data
    Remove-Item -Recurse -Force node1\data, node2\data, node3\data -ErrorAction SilentlyContinue

    # Start nodes
    Start-Process -FilePath ".\target\release\luxtensor-node.exe" -ArgumentList "--config config.toml" -WorkingDirectory ".\node1" -WindowStyle Hidden
    Start-Sleep -Seconds 3
    Start-Process -FilePath ".\target\release\luxtensor-node.exe" -ArgumentList "--config config.toml" -WorkingDirectory ".\node2" -WindowStyle Hidden
    Start-Sleep -Seconds 3
    Start-Process -FilePath ".\target\release\luxtensor-node.exe" -ArgumentList "--config config.toml" -WorkingDirectory ".\node3" -WindowStyle Hidden

    Write-Host "✅ Nodes started" -ForegroundColor Green
    Write-Host "   Waiting 30s for sync..." -ForegroundColor Gray
    Start-Sleep -Seconds 30
}

# Run tests based on mode
switch ($Mode) {
    "unit" {
        Write-Host "═══ Running Unit Tests ═══" -ForegroundColor Cyan
        cargo test --workspace --lib
    }
    "rpc" {
        Write-Host "═══ Running RPC API Tests ═══" -ForegroundColor Cyan
        Write-Host "⚠️  Make sure Node 1 is running on port 8545" -ForegroundColor Yellow
        cargo test --package luxtensor-tests --test rpc_tests -- $(if ($Verbose) { "--nocapture" })
    }
    "network" {
        Write-Host "═══ Running Network Tests ═══" -ForegroundColor Cyan
        Write-Host "⚠️  Make sure all 3 nodes are running" -ForegroundColor Yellow
        cargo test --package luxtensor-tests --test network_tests -- --ignored $(if ($Verbose) { "--nocapture" })
    }
    "bench" {
        Write-Host "═══ Running Benchmarks ═══" -ForegroundColor Cyan
        cargo bench --package luxtensor-tests
    }
    "integration" {
        Write-Host "═══ Running Integration Tests ═══" -ForegroundColor Cyan
        cargo test --package luxtensor-tests --test integration_tests
    }
    "all" {
        Write-Host "═══ Running All Tests ═══" -ForegroundColor Cyan

        Write-Host "`n--- Unit Tests ---" -ForegroundColor Magenta
        cargo test --workspace --lib

        Write-Host "`n--- Integration Tests ---" -ForegroundColor Magenta
        cargo test --package luxtensor-tests --test integration_tests

        if ($StartNodes) {
            Write-Host "`n--- RPC Tests ---" -ForegroundColor Magenta
            cargo test --package luxtensor-tests --test rpc_tests

            Write-Host "`n--- Network Tests ---" -ForegroundColor Magenta
            cargo test --package luxtensor-tests --test network_tests -- --ignored
        } else {
            Write-Host "`n⚠️  Skipping RPC and Network tests (use -StartNodes to include)" -ForegroundColor Yellow
        }
    }
    default {
        Write-Host "Unknown mode: $Mode" -ForegroundColor Red
        Write-Host "Valid modes: all, unit, rpc, network, bench, integration"
        exit 1
    }
}

Write-Host ""
Write-Host "═══════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "✅ Test run complete!" -ForegroundColor Green
