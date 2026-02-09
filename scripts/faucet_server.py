#!/usr/bin/env python3
"""
LuxTensor Testnet Faucet Server
================================
A lightweight faucet web server using only the Python standard library + requests.

Endpoints:
    GET  /        → HTML form to request testnet tokens
    POST /faucet  → JSON API: {"address": "0x..."} → credits tokens via dev_faucet RPC

Rate limit: 1 request per address per hour.

Usage:
    python3 scripts/faucet_server.py                          # defaults
    python3 scripts/faucet_server.py --port 9090              # custom port
    python3 scripts/faucet_server.py --rpc http://127.0.0.1:8546  # custom RPC
"""

import argparse
import json
import re
import time
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse

try:
    import requests
except ImportError:
    raise SystemExit(
        "Missing dependency: requests\n"
        "Install with:  pip install requests"
    )

# ── Configuration ─────────────────────────────────────────────────────────────
DEFAULT_PORT = 8080
DEFAULT_RPC_URL = "http://127.0.0.1:8545"
DEFAULT_AMOUNT = "1000000000000"  # 1000 MDT in base units
RATE_LIMIT_SECONDS = 3600  # 1 hour

# Track last request time per address
_rate_limit_store: dict[str, float] = {}

# ── HTML Template ─────────────────────────────────────────────────────────────
HTML_PAGE = """\
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>LuxTensor Testnet Faucet</title>
<style>
  *, *::before, *::after { box-sizing: border-box; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #0f172a; color: #e2e8f0;
    display: flex; justify-content: center; align-items: center;
    min-height: 100vh; margin: 0;
  }
  .card {
    background: #1e293b; border-radius: 16px; padding: 2.5rem;
    max-width: 480px; width: 100%; box-shadow: 0 25px 50px rgba(0,0,0,.4);
  }
  h1 { margin: 0 0 .25rem; font-size: 1.6rem; color: #38bdf8; }
  .subtitle { color: #94a3b8; margin-bottom: 1.5rem; font-size: .9rem; }
  label { display: block; font-weight: 600; margin-bottom: .4rem; font-size: .85rem; }
  input[type=text] {
    width: 100%; padding: .7rem .9rem; border: 2px solid #334155;
    border-radius: 8px; background: #0f172a; color: #e2e8f0;
    font-family: monospace; font-size: .95rem;
    outline: none; transition: border-color .2s;
  }
  input[type=text]:focus { border-color: #38bdf8; }
  button {
    margin-top: 1rem; width: 100%; padding: .75rem;
    background: #2563eb; color: #fff; border: none; border-radius: 8px;
    font-size: 1rem; font-weight: 600; cursor: pointer;
    transition: background .2s;
  }
  button:hover { background: #1d4ed8; }
  button:disabled { background: #475569; cursor: not-allowed; }
  #result {
    margin-top: 1.2rem; padding: .8rem; border-radius: 8px;
    font-size: .85rem; display: none; word-break: break-all;
  }
  .success { background: #064e3b; border: 1px solid #059669; display: block !important; }
  .error   { background: #7f1d1d; border: 1px solid #dc2626; display: block !important; }
  .info    { color: #64748b; font-size: .75rem; margin-top: 1rem; text-align: center; }
</style>
</head>
<body>
<div class="card">
  <h1>&#x1f4a7; LuxTensor Faucet</h1>
  <p class="subtitle">Get testnet MDT tokens &mdash; Chain ID 9999</p>
  <form id="faucetForm">
    <label for="address">Wallet Address</label>
    <input type="text" id="address" name="address"
           placeholder="0x0000000000000000000000000000000000000000" required>
    <button type="submit" id="btn">Request 1000 MDT</button>
  </form>
  <div id="result"></div>
  <p class="info">Rate limit: 1 request per address per hour.</p>
</div>
<script>
  const form = document.getElementById('faucetForm');
  const btn  = document.getElementById('btn');
  const res  = document.getElementById('result');

  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const addr = document.getElementById('address').value.trim();
    if (!/^0x[0-9a-fA-F]{40}$/.test(addr)) {
      res.className = 'error'; res.style.display = 'block';
      res.textContent = 'Invalid Ethereum address format.';
      return;
    }
    btn.disabled = true; btn.textContent = 'Sending...';
    res.style.display = 'none';
    try {
      const resp = await fetch('/faucet', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({address: addr})
      });
      const data = await resp.json();
      if (data.success) {
        res.className = 'success'; res.style.display = 'block';
        res.textContent = `Sent ${data.credited || '1000'} MDT to ${data.address}. New balance: ${data.new_balance}`;
      } else {
        res.className = 'error'; res.style.display = 'block';
        res.textContent = data.error || 'Request failed.';
      }
    } catch(err) {
      res.className = 'error'; res.style.display = 'block';
      res.textContent = 'Network error: ' + err.message;
    } finally {
      btn.disabled = false; btn.textContent = 'Request 1000 MDT';
    }
  });
</script>
</body>
</html>
"""

# ── Helpers ───────────────────────────────────────────────────────────────────
ADDRESS_RE = re.compile(r"^0x[0-9a-fA-F]{40}$")


def is_valid_address(addr: str) -> bool:
    return bool(ADDRESS_RE.match(addr))


def check_rate_limit(address: str) -> tuple[bool, int]:
    """Returns (allowed, seconds_remaining)."""
    addr = address.lower()
    now = time.time()
    last = _rate_limit_store.get(addr, 0.0)
    elapsed = now - last
    if elapsed < RATE_LIMIT_SECONDS:
        remaining = int(RATE_LIMIT_SECONDS - elapsed)
        return False, remaining
    return True, 0


def record_request(address: str) -> None:
    _rate_limit_store[address.lower()] = time.time()


def call_dev_faucet(rpc_url: str, address: str, amount: str) -> dict:
    """Call dev_faucet JSON-RPC method on the node."""
    payload = {
        "jsonrpc": "2.0",
        "method": "dev_faucet",
        "params": [address, amount],
        "id": 1,
    }
    try:
        resp = requests.post(rpc_url, json=payload, timeout=10)
        resp.raise_for_status()
        data = resp.json()
        if "error" in data:
            return {"success": False, "error": data["error"].get("message", str(data["error"]))}
        result = data.get("result", {})
        return {
            "success": True,
            "address": result.get("address", address),
            "credited": result.get("credited", amount),
            "new_balance": result.get("new_balance", "unknown"),
        }
    except requests.ConnectionError:
        return {"success": False, "error": "Cannot connect to node RPC — is the testnet running?"}
    except requests.Timeout:
        return {"success": False, "error": "Node RPC request timed out."}
    except Exception as exc:
        return {"success": False, "error": f"RPC call failed: {exc}"}


# ── HTTP Handler ──────────────────────────────────────────────────────────────
class FaucetHandler(BaseHTTPRequestHandler):
    """Simple HTTP handler for the faucet server."""

    rpc_url: str = DEFAULT_RPC_URL
    faucet_amount: str = DEFAULT_AMOUNT

    def log_message(self, format, *args):  # noqa: A002
        # Prefix log lines for clarity
        print(f"[faucet] {args[0]} {args[1]} {args[2]}")

    # ── GET / ──────────────────────────────────────────────────────────────
    def do_GET(self):  # noqa: N802
        parsed = urlparse(self.path)
        if parsed.path == "/":
            self._send_html(200, HTML_PAGE)
        elif parsed.path == "/health":
            self._send_json(200, {"status": "ok"})
        else:
            self._send_json(404, {"error": "Not found"})

    # ── POST /faucet ───────────────────────────────────────────────────────
    def do_POST(self):  # noqa: N802
        parsed = urlparse(self.path)
        if parsed.path != "/faucet":
            self._send_json(404, {"error": "Not found"})
            return

        # Read body
        content_length = int(self.headers.get("Content-Length", 0))
        if content_length == 0 or content_length > 4096:
            self._send_json(400, {"success": False, "error": "Invalid request body."})
            return

        try:
            body = json.loads(self.rfile.read(content_length))
        except (json.JSONDecodeError, ValueError):
            self._send_json(400, {"success": False, "error": "Invalid JSON."})
            return

        address = body.get("address", "").strip()

        # Validate address
        if not is_valid_address(address):
            self._send_json(400, {
                "success": False,
                "error": "Invalid address. Must be 0x-prefixed, 40 hex chars.",
            })
            return

        # Rate limit
        allowed, wait = check_rate_limit(address)
        if not allowed:
            self._send_json(429, {
                "success": False,
                "error": f"Rate limited. Try again in {wait} seconds.",
            })
            return

        # Call RPC
        result = call_dev_faucet(self.rpc_url, address, self.faucet_amount)

        if result["success"]:
            record_request(address)
            self._send_json(200, result)
        else:
            self._send_json(502, result)

    # ── Response helpers ───────────────────────────────────────────────────
    def _send_json(self, code: int, data: dict) -> None:
        body = json.dumps(data).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body)

    def _send_html(self, code: int, html: str) -> None:
        body = html.encode()
        self.send_response(code)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


# ── Main ──────────────────────────────────────────────────────────────────────
def main() -> None:
    parser = argparse.ArgumentParser(description="LuxTensor Testnet Faucet Server")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT, help="HTTP listen port")
    parser.add_argument("--rpc", default=DEFAULT_RPC_URL, help="Node JSON-RPC URL")
    parser.add_argument("--amount", default=DEFAULT_AMOUNT, help="Faucet amount in base units")
    args = parser.parse_args()

    FaucetHandler.rpc_url = args.rpc
    FaucetHandler.faucet_amount = args.amount

    server = HTTPServer(("0.0.0.0", args.port), FaucetHandler)
    print(f"[faucet] LuxTensor Testnet Faucet running on http://0.0.0.0:{args.port}")
    print(f"[faucet] RPC endpoint: {args.rpc}")
    print(f"[faucet] Faucet amount: {args.amount} base units")
    print(f"[faucet] Rate limit: 1 request / address / {RATE_LIMIT_SECONDS}s")
    print(f"[faucet] Press Ctrl+C to stop.")

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[faucet] Shutting down.")
        server.server_close()


if __name__ == "__main__":
    main()
