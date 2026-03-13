"""
UPP SDK WebSocket Client

Real-time subscriptions to market prices and orderbook updates.
Includes automatic reconnection with exponential backoff.
"""

import asyncio
import json
from typing import Any, Callable, Dict, Optional

import websockets
from websockets.client import WebSocketClientProtocol

from .types import (
    JsonRpcRequest,
    JsonRpcResponse,
    MarketStatus,
    OrderbookSubscription,
    OrderbookUpdate,
    PriceSubscription,
    PriceUpdate,
)


class UppWebSocket:
    """
    WebSocket client for real-time market data.

    Example:
        >>> ws = UppWebSocket(url='ws://localhost:8080/upp/v1/ws')
        >>> ws.on_price(lambda u: print(f'Price: {u}'))
        >>> await ws.connect()
        >>> await ws.subscribe_prices(['upp:kalshi:MELON-240301'])
        >>> # ... later ...
        >>> ws.disconnect()
    """

    def __init__(
        self,
        url: str,
        reconnect: bool = True,
        max_reconnect_attempts: int = 10,
        initial_reconnect_delay: float = 1.0,
        max_reconnect_delay: float = 30.0,
        reconnect_backoff_multiplier: float = 2.0,
        heartbeat_interval: float = 30.0,
    ):
        """
        Initialize the WebSocket client.

        Args:
            url: WebSocket URL (e.g., 'ws://localhost:8080/upp/v1/ws')
            reconnect: Enable automatic reconnection
            max_reconnect_attempts: Maximum reconnection attempts
            initial_reconnect_delay: Initial reconnection delay in seconds
            max_reconnect_delay: Maximum reconnection delay in seconds
            reconnect_backoff_multiplier: Backoff multiplier for reconnection
            heartbeat_interval: Heartbeat interval in seconds
        """
        self.url = url
        self.reconnect_enabled = reconnect
        self.max_reconnect_attempts = max_reconnect_attempts
        self.initial_reconnect_delay = initial_reconnect_delay
        self.max_reconnect_delay = max_reconnect_delay
        self.reconnect_backoff_multiplier = reconnect_backoff_multiplier
        self.heartbeat_interval = heartbeat_interval

        self.socket: Optional[WebSocketClientProtocol] = None
        self.connected = False
        self.reconnect_attempts = 0

        self._on_connect: Optional[Callable[[], None]] = None
        self._on_disconnect: Optional[Callable[[], None]] = None
        self._on_price: Optional[Callable[[PriceUpdate], None]] = None
        self._on_orderbook: Optional[Callable[[OrderbookUpdate], None]] = None
        self._on_error: Optional[Callable[[Exception], None]] = None

        self._request_id = 0
        self._heartbeat_task: Optional[asyncio.Task[Any]] = None
        self._reconnect_task: Optional[asyncio.Task[Any]] = None
        self._receive_task: Optional[asyncio.Task[Any]] = None

    def on_connect(self, callback: Callable[[], None]) -> "UppWebSocket":
        """Register connect callback."""
        self._on_connect = callback
        return self

    def on_disconnect(self, callback: Callable[[], None]) -> "UppWebSocket":
        """Register disconnect callback."""
        self._on_disconnect = callback
        return self

    def on_price(
        self, callback: Callable[[PriceUpdate], None]
    ) -> "UppWebSocket":
        """Register price update callback."""
        self._on_price = callback
        return self

    def on_orderbook(
        self, callback: Callable[[OrderbookUpdate], None]
    ) -> "UppWebSocket":
        """Register orderbook update callback."""
        self._on_orderbook = callback
        return self

    def on_error(
        self, callback: Callable[[Exception], None]
    ) -> "UppWebSocket":
        """Register error callback."""
        self._on_error = callback
        return self

    async def connect(self) -> None:
        """Connect to the WebSocket server."""
        try:
            self.socket = await websockets.connect(self.url)
            self.connected = True
            self.reconnect_attempts = 0

            if self._on_connect:
                self._on_connect()

            # Start heartbeat and receive tasks
            self._heartbeat_task = asyncio.create_task(self._heartbeat_loop())
            self._receive_task = asyncio.create_task(self._receive_loop())
        except Exception as e:
            self.connected = False
            if self._on_error:
                self._on_error(e)
            await self._attempt_reconnect()

    def disconnect(self) -> None:
        """Disconnect from the WebSocket server."""
        self.connected = False

        # Cancel all tasks
        if self._heartbeat_task:
            self._heartbeat_task.cancel()
        if self._receive_task:
            self._receive_task.cancel()
        if self._reconnect_task:
            self._reconnect_task.cancel()

        # Close socket
        if self.socket:
            asyncio.create_task(self.socket.close())

    async def subscribe_prices(
        self, market_ids: list[str], interval_ms: int = 1000
    ) -> None:
        """
        Subscribe to price updates for specified markets.

        Args:
            market_ids: Market IDs to subscribe to
            interval_ms: Update interval in milliseconds
        """
        self._request_id += 1
        request = JsonRpcRequest(
            id=self._request_id,
            method="subscribe_prices",
            params={
                "channel": "prices",
                "market_ids": market_ids,
                "interval_ms": interval_ms,
            },
        )
        await self._send_request(request)

    async def subscribe_orderbook(
        self,
        market_ids: list[str],
        depth: int = 10,
        interval_ms: int = 2000,
    ) -> None:
        """
        Subscribe to orderbook updates for specified markets.

        Args:
            market_ids: Market IDs to subscribe to
            depth: Orderbook depth
            interval_ms: Update interval in milliseconds
        """
        self._request_id += 1
        request = JsonRpcRequest(
            id=self._request_id,
            method="subscribe_orderbook",
            params={
                "channel": "orderbook",
                "market_ids": market_ids,
                "depth": depth,
                "interval_ms": interval_ms,
            },
        )
        await self._send_request(request)

    async def unsubscribe(
        self, channel: str, market_ids: list[str]
    ) -> None:
        """
        Unsubscribe from a channel.

        Args:
            channel: Channel name ('prices' or 'orderbook')
            market_ids: Market IDs to unsubscribe from
        """
        self._request_id += 1
        request = JsonRpcRequest(
            id=self._request_id,
            method="unsubscribe",
            params={
                "channel": channel,
                "market_ids": market_ids,
            },
        )
        await self._send_request(request)

    async def get_market(self, market_id: str) -> Dict[str, Any]:
        """Get a specific market (one-off request)."""
        self._request_id += 1
        request = JsonRpcRequest(
            id=self._request_id,
            method="get_market",
            params={"market_id": market_id},
        )
        return await self._send_rpc_request(request)

    def is_connected(self) -> bool:
        """Check if WebSocket is connected."""
        return (
            self.connected
            and self.socket is not None
            and not self.socket.closed
        )

    # ─── Private Methods ────────────────────────────────────

    async def _send_request(self, request: JsonRpcRequest) -> None:
        """Send a JSON-RPC request."""
        if not self.is_connected():
            raise RuntimeError("WebSocket not connected")

        message = json.dumps(request.model_dump(exclude_none=True))
        await self.socket.send(message)

    async def _send_rpc_request(
        self, request: JsonRpcRequest
    ) -> Dict[str, Any]:
        """Send a JSON-RPC request and wait for response."""
        if not self.is_connected():
            raise RuntimeError("WebSocket not connected")

        message = json.dumps(request.model_dump(exclude_none=True))
        await self.socket.send(message)

        # Wait for response with timeout
        try:
            async for msg in self.socket:
                response = json.loads(msg)
                if response.get("id") == request.id:
                    rpc_response = JsonRpcResponse.model_validate(response)
                    if rpc_response.error:
                        raise RuntimeError(rpc_response.error["message"])
                    return rpc_response.result
        except asyncio.TimeoutError:
            raise RuntimeError(f"Request {request.id} timed out")

    async def _heartbeat_loop(self) -> None:
        """Send periodic heartbeat pings."""
        while self.is_connected():
            try:
                await asyncio.sleep(self.heartbeat_interval)
                if self.is_connected():
                    self._request_id += 1
                    request = JsonRpcRequest(
                        id=self._request_id, method="ping", params={}
                    )
                    await self._send_request(request)
            except Exception as e:
                if self._on_error:
                    self._on_error(e)

    async def _receive_loop(self) -> None:
        """Receive and handle messages from the WebSocket."""
        try:
            async for msg in self.socket:
                try:
                    data = json.loads(msg)

                    # Check if it's a fan-out message or JSON-RPC response
                    if "channel" in data:
                        if data["channel"] == "prices":
                            update = PriceUpdate.model_validate(data)
                            if self._on_price:
                                self._on_price(update)
                        elif data["channel"] == "orderbook":
                            update = OrderbookUpdate.model_validate(data)
                            if self._on_orderbook:
                                self._on_orderbook(update)
                except Exception as e:
                    if self._on_error:
                        self._on_error(e)
        except asyncio.CancelledError:
            pass
        except Exception as e:
            if self._on_error:
                self._on_error(e)
            await self._attempt_reconnect()

    async def _attempt_reconnect(self) -> None:
        """Attempt to reconnect with exponential backoff."""
        if not self.reconnect_enabled:
            return
        if self.reconnect_attempts >= self.max_reconnect_attempts:
            if self._on_error:
                self._on_error(
                    RuntimeError(
                        f"Failed to reconnect after {self.reconnect_attempts} attempts"
                    )
                )
            return

        delay = min(
            self.initial_reconnect_delay
            * (
                self.reconnect_backoff_multiplier
                ** self.reconnect_attempts
            ),
            self.max_reconnect_delay,
        )

        self.reconnect_attempts += 1
        await asyncio.sleep(delay)

        try:
            await self.connect()
        except Exception as e:
            if self._on_error:
                self._on_error(e)
            await self._attempt_reconnect()
