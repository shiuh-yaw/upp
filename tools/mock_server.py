#!/usr/bin/env python3
"""
UPP Mock Server — Simulates auth-required endpoints for local development.

Provides fake trading, portfolio, and resolution endpoints so you can test
the full UPP flow without real API credentials or real money.

Usage:
    pip install flask
    python tools/mock_server.py

Runs on http://localhost:8081 by default.
"""

import json
import uuid
import time
from datetime import datetime, timezone
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs

PORT = 8081

# ─── In-Memory State ─────────────────────────────────────────

orders = {}
positions = {}
trades = []
balances = {
    "kalshi.com": {"available": "10000.00", "reserved": "0.00", "currency": "USD"},
    "polymarket.com": {"available": "5000.00", "reserved": "0.00", "currency": "USDC"},
    "opinion.trade": {"available": "2000.00", "reserved": "0.00", "currency": "USDC"},
}

# Sample markets for mock order placement
mock_markets = {
    "PRES-2028-DEM": {"title": "Democratic nominee 2028", "yes_price": "0.35", "provider": "kalshi.com"},
    "BTC-100K": {"title": "Bitcoin to $100K", "yes_price": "0.65", "provider": "polymarket.com"},
}


def now_iso():
    return datetime.now(timezone.utc).isoformat()


def upp_response(data, status=200):
    return status, json.dumps(data, indent=2)


def error_response(code, message, status=400):
    return status, json.dumps({
        "error": {
            "code": code,
            "message": message,
            "request_id": str(uuid.uuid4()),
        }
    }, indent=2)


# ─── Request Handler ─────────────────────────────────────────

class MockHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urlparse(self.path)
        path = parsed.path
        params = parse_qs(parsed.query)

        if path == "/health":
            self._respond(*upp_response({"status": "mock_server", "healthy": True}))

        elif path == "/v1/providers":
            self._respond(*upp_response([
                {"id": "kalshi.com", "name": "Kalshi (Mock)", "status": "operational"},
                {"id": "polymarket.com", "name": "Polymarket (Mock)", "status": "operational"},
                {"id": "opinion.trade", "name": "Opinion (Mock)", "status": "operational"},
            ]))

        elif path.startswith("/v1/orders"):
            parts = path.split("/")
            if len(parts) == 4:  # /v1/orders/{id}
                order_id = parts[3]
                if order_id in orders:
                    self._respond(*upp_response(orders[order_id]))
                else:
                    self._respond(*error_response("NOT_FOUND", f"Order {order_id} not found", 404))
            else:
                # List orders
                provider = params.get("provider", [None])[0]
                filtered = list(orders.values())
                if provider:
                    filtered = [o for o in filtered if o.get("provider") == provider]
                self._respond(*upp_response({
                    "orders": filtered,
                    "pagination": {"cursor": "", "has_more": False, "total": len(filtered)},
                }))

        elif path.startswith("/v1/positions"):
            provider = params.get("provider", [None])[0]
            pos_list = list(positions.values())
            if provider:
                pos_list = [p for p in pos_list if p.get("provider") == provider]
            self._respond(*upp_response({
                "positions": pos_list,
                "pagination": {"cursor": "", "has_more": False, "total": len(pos_list)},
            }))

        elif path.startswith("/v1/balances"):
            balance_list = []
            for prov, bal in balances.items():
                balance_list.append({
                    "provider": prov,
                    "instrument_type": "cash",
                    "available": bal["available"],
                    "reserved": bal["reserved"],
                    "total": str(float(bal["available"]) + float(bal["reserved"])),
                    "currency": bal["currency"],
                })
            self._respond(*upp_response({"balances": balance_list}))

        elif path.startswith("/v1/trades"):
            provider = params.get("provider", [None])[0]
            filtered = trades
            if provider:
                filtered = [t for t in filtered if t.get("provider") == provider]
            self._respond(*upp_response({
                "trades": filtered[-50:],  # Last 50
                "pagination": {"cursor": "", "has_more": False, "total": len(filtered)},
            }))

        else:
            self._respond(*error_response("NOT_FOUND", f"Unknown endpoint: {path}", 404))

    def do_POST(self):
        parsed = urlparse(self.path)
        path = parsed.path
        content_length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(content_length)) if content_length > 0 else {}

        if path == "/v1/orders":
            # Create order
            order_id = str(uuid.uuid4())[:8]
            provider = body.get("provider", "kalshi.com")
            market_id = body.get("market_id", "UNKNOWN")
            outcome_id = body.get("outcome_id", "yes")
            side = body.get("side", "buy")
            price = body.get("price", "0.50")
            quantity = body.get("quantity", 10)
            order_type = body.get("order_type", "limit")

            # Simulate fill
            filled = quantity if order_type == "market" else 0
            status = "filled" if filled == quantity else "open"

            order = {
                "id": order_id,
                "provider_order_id": f"mock-{order_id}",
                "market_id": {
                    "provider": provider,
                    "native_id": market_id,
                    "full_id": f"upp:{provider}:{market_id}",
                },
                "outcome_id": outcome_id,
                "side": side,
                "order_type": order_type,
                "tif": body.get("tif", "GTC"),
                "price": price,
                "quantity": quantity,
                "filled_quantity": filled,
                "remaining_quantity": quantity - filled,
                "status": status,
                "fees": {"maker_fee": "0.00", "taker_fee": "0.00", "total_fee": "0.00"},
                "created_at": now_iso(),
                "updated_at": now_iso(),
                "provider": provider,
            }
            orders[order_id] = order

            # Update balance
            cost = float(price) * quantity
            if provider in balances:
                avail = float(balances[provider]["available"])
                if cost > avail:
                    self._respond(*error_response("INSUFFICIENT_FUNDS", f"Need {cost}, have {avail}"))
                    return
                balances[provider]["available"] = f"{avail - cost:.2f}"
                balances[provider]["reserved"] = f"{cost:.2f}"

            # If filled, create trade and position
            if status == "filled":
                trade = {
                    "id": str(uuid.uuid4())[:8],
                    "order_id": order_id,
                    "market_id": order["market_id"],
                    "outcome_id": outcome_id,
                    "side": side,
                    "price": price,
                    "quantity": quantity,
                    "notional": f"{cost:.2f}",
                    "role": "taker",
                    "fees": order["fees"],
                    "executed_at": now_iso(),
                    "provider": provider,
                }
                trades.append(trade)

                pos_key = f"{provider}:{market_id}:{outcome_id}"
                if pos_key in positions:
                    pos = positions[pos_key]
                    pos["quantity"] += quantity if side == "buy" else -quantity
                    pos["updated_at"] = now_iso()
                else:
                    positions[pos_key] = {
                        "market_id": order["market_id"],
                        "outcome_id": outcome_id,
                        "quantity": quantity if side == "buy" else -quantity,
                        "average_entry_price": price,
                        "current_price": price,
                        "cost_basis": f"{cost:.2f}",
                        "current_value": f"{cost:.2f}",
                        "unrealized_pnl": "0.00",
                        "realized_pnl": "0.00",
                        "status": "open",
                        "opened_at": now_iso(),
                        "updated_at": now_iso(),
                        "market_title": mock_markets.get(market_id, {}).get("title", market_id),
                        "provider": provider,
                    }

            self._respond(*upp_response(order, 201))

        elif path.startswith("/v1/orders/") and path.endswith("/cancel"):
            # Cancel order
            order_id = path.split("/")[3]
            if order_id in orders:
                order = orders[order_id]
                if order["status"] in ("open", "partially_filled"):
                    order["status"] = "cancelled"
                    order["cancelled_at"] = now_iso()
                    order["updated_at"] = now_iso()
                    order["remaining_quantity"] = 0

                    # Release reserved funds
                    provider = order.get("provider", "kalshi.com")
                    if provider in balances:
                        reserved = float(balances[provider]["reserved"])
                        cost = float(order["price"]) * order["quantity"]
                        balances[provider]["available"] = f"{float(balances[provider]['available']) + cost:.2f}"
                        balances[provider]["reserved"] = f"{max(0, reserved - cost):.2f}"

                    self._respond(*upp_response(order))
                else:
                    self._respond(*error_response("INVALID_STATE", f"Cannot cancel {order['status']} order"))
            else:
                self._respond(*error_response("NOT_FOUND", f"Order {order_id} not found", 404))

        else:
            self._respond(*error_response("NOT_FOUND", f"Unknown endpoint: {path}", 404))

    def do_DELETE(self):
        parsed = urlparse(self.path)
        path = parsed.path

        if path == "/v1/orders":
            # Cancel all orders
            cancelled = []
            for oid, order in orders.items():
                if order["status"] in ("open", "partially_filled"):
                    order["status"] = "cancelled"
                    order["cancelled_at"] = now_iso()
                    cancelled.append(oid)
            self._respond(*upp_response({"cancelled": cancelled, "count": len(cancelled)}))
        else:
            self._respond(*error_response("NOT_FOUND", f"Unknown endpoint: {path}", 404))

    def _respond(self, status, body):
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body.encode())

    def log_message(self, format, *args):
        print(f"[MOCK] {args[0]}")


# ─── Main ────────────────────────────────────────────────────

if __name__ == "__main__":
    print(f"""
╔═══════════════════════════════════════════════════════════╗
║            UPP Mock Server — Local Development            ║
╠═══════════════════════════════════════════════════════════╣
║                                                           ║
║  Endpoints:                                               ║
║    POST /v1/orders          — Create order                ║
║    GET  /v1/orders          — List orders                 ║
║    GET  /v1/orders/{{id}}     — Get order                  ║
║    POST /v1/orders/{{id}}/cancel — Cancel order            ║
║    DELETE /v1/orders        — Cancel all orders            ║
║    GET  /v1/positions       — List positions               ║
║    GET  /v1/balances        — Get balances                 ║
║    GET  /v1/trades          — List trades                  ║
║    GET  /v1/providers       — List providers               ║
║    GET  /health             — Health check                 ║
║                                                           ║
║  Mock balances:                                           ║
║    Kalshi:      $10,000 USD                               ║
║    Polymarket:  $5,000 USDC                               ║
║    Opinion:     $2,000 USDC                               ║
║                                                           ║
║  Running on http://localhost:{PORT}                         ║
╚═══════════════════════════════════════════════════════════╝
""")
    server = HTTPServer(("0.0.0.0", PORT), MockHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down mock server...")
        server.shutdown()
