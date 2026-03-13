"""
UPP SDK Python Client

Main client for interacting with the UPP Gateway REST API.
Provides typed methods for all endpoints and automatic error handling.
"""

from typing import Any, Dict, Optional, Type, TypeVar

import httpx

from .types import (
    CancelAllOrdersResponse,
    CreateOrderRequest,
    EstimateOrderRequest,
    HealthStatus,
    MarketStatus,
    MarketsResponse,
    McpExecuteResponse,
    McpSchemaResponse,
    McpToolsResponse,
    OrderEstimate,
    Order,
    OrderbookResponse,
    OrdersResponse,
    OrderStatus,
    PortfolioBalance,
    PortfolioBalancesResponse,
    PortfolioSummary,
    PositionsResponse,
    ProviderManifest,
    Market,
    MarketType,
    TradesResponse,
    UppError,
    WellKnown,
    AgentCard,
    MergedOrderbookResponse,
)

T = TypeVar("T")


class UppApiError(Exception):
    """Error thrown by the UPP API client."""

    def __init__(
        self,
        message: str,
        code: str,
        status: int,
        details: Optional[Dict[str, str]] = None,
    ):
        super().__init__(message)
        self.message = message
        self.code = code
        self.status = status
        self.details = details or {}


class UppClient:
    """
    Main UPP Client class for REST API interactions.

    Example:
        >>> client = UppClient(
        ...     base_url='http://localhost:8080',
        ...     api_key='your-api-key'
        ... )
        >>> market = await client.get_market('upp:kalshi:MELON-240301')
        >>> orders = await client.list_orders()
    """

    def __init__(
        self,
        base_url: str,
        api_key: Optional[str] = None,
        timeout: float = 30.0,
    ):
        """
        Initialize the UPP client.

        Args:
            base_url: Base URL of the UPP Gateway (e.g., 'http://localhost:8080')
            api_key: Optional API key for authenticated requests
            timeout: Request timeout in seconds (default: 30)
        """
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.timeout = timeout
        self._client = httpx.AsyncClient(
            timeout=httpx.Timeout(timeout),
            headers={"Content-Type": "application/json"},
        )

    async def _request(
        self,
        method: str,
        path: str,
        *,
        params: Optional[Dict[str, Any]] = None,
        json: Optional[Dict[str, Any]] = None,
        auth: bool = False,
    ) -> Any:
        """Make an HTTP request to the UPP Gateway."""
        url = f"{self.base_url}{path}"

        headers = {}
        if auth and self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"

        try:
            response = await self._client.request(
                method,
                url,
                params=params,
                json=json,
                headers=headers,
            )

            if not response.is_success:
                try:
                    error = UppError.model_validate(response.json())
                    raise UppApiError(
                        error.error.message,
                        error.error.code,
                        response.status_code,
                        error.error.details,
                    )
                except Exception as e:
                    if isinstance(e, UppApiError):
                        raise
                    raise UppApiError(
                        response.text,
                        "UNKNOWN",
                        response.status_code,
                    )

            return response.json()
        except httpx.RequestError as e:
            raise UppApiError(str(e), "REQUEST_ERROR", 0)

    # ─── Health & Metrics ────────────────────────────────────

    async def health(self) -> Dict[str, Any]:
        """Check if the gateway is healthy."""
        return await self._request("GET", "/health")

    async def ready(self) -> Dict[str, Any]:
        """Check if the gateway is ready to serve requests."""
        return await self._request("GET", "/ready")

    async def metrics(self) -> str:
        """Get metrics from the gateway."""
        response = await self._client.get(f"{self.base_url}/metrics")
        return response.text

    # ─── Discovery ──────────────────────────────────────────

    async def get_well_known(self) -> WellKnown:
        """Get the well-known UPP endpoint."""
        data = await self._request("GET", "/.well-known/upp")
        return WellKnown.model_validate(data)

    async def list_providers(self) -> Dict[str, Any]:
        """List all available prediction market providers."""
        return await self._request("GET", "/upp/v1/discovery/providers")

    async def get_manifest(self, provider: str) -> ProviderManifest:
        """Get the manifest for a specific provider."""
        data = await self._request(
            "GET", f"/upp/v1/discovery/manifest/{provider}"
        )
        return ProviderManifest.model_validate(data)

    async def negotiate(self, provider: str) -> Dict[str, Any]:
        """Negotiate capabilities with a provider."""
        return await self._request(
            "POST",
            "/upp/v1/discovery/negotiate",
            json={"provider": provider},
        )

    async def check_provider_health(self, provider: str) -> HealthStatus:
        """Check health of a specific provider."""
        data = await self._request(
            "GET", f"/upp/v1/discovery/health/{provider}"
        )
        return HealthStatus.model_validate(data)

    async def check_all_provider_health(self) -> Dict[str, Any]:
        """Check health of all providers."""
        return await self._request("GET", "/upp/v1/discovery/health")

    # ─── Markets ────────────────────────────────────────────

    async def list_markets(
        self,
        provider: Optional[str] = None,
        status: Optional[MarketStatus] = None,
        category: Optional[str] = None,
        market_type: Optional[MarketType] = None,
        sort_by: Optional[str] = None,
        limit: Optional[int] = None,
        cursor: Optional[str] = None,
    ) -> MarketsResponse:
        """
        List markets with optional filtering.

        Args:
            provider: Filter by provider
            status: Filter by market status
            category: Filter by category
            market_type: Filter by market type
            sort_by: Sort by field
            limit: Page limit
            cursor: Pagination cursor

        Returns:
            Markets list response
        """
        params = {
            k: v
            for k, v in {
                "provider": provider,
                "status": status,
                "category": category,
                "market_type": market_type,
                "sort_by": sort_by,
                "limit": limit,
                "cursor": cursor,
            }.items()
            if v is not None
        }

        data = await self._request("GET", "/upp/v1/markets", params=params)
        return MarketsResponse.model_validate(data)

    async def search_markets(
        self,
        query: str,
        provider: Optional[str] = None,
        limit: Optional[int] = None,
        cursor: Optional[str] = None,
    ) -> MarketsResponse:
        """
        Search markets by query string.

        Args:
            query: Search query
            provider: Filter by provider
            limit: Page limit
            cursor: Pagination cursor

        Returns:
            Markets list response
        """
        params = {
            "q": query,
        }
        if provider:
            params["provider"] = provider
        if limit:
            params["limit"] = limit
        if cursor:
            params["cursor"] = cursor

        data = await self._request(
            "GET", "/upp/v1/markets/search", params=params
        )
        return MarketsResponse.model_validate(data)

    async def get_market(self, market_id: str) -> Market:
        """
        Get a specific market by ID.

        Args:
            market_id: Market ID (e.g., 'upp:kalshi:MELON-240301')

        Returns:
            Market data
        """
        data = await self._request("GET", f"/upp/v1/markets/{market_id}")
        return Market.model_validate(data)

    async def get_orderbook(
        self,
        market_id: str,
        outcome: Optional[str] = None,
        depth: Optional[int] = None,
    ) -> OrderbookResponse:
        """
        Get order book for a market.

        Args:
            market_id: Market ID
            outcome: Filter by outcome
            depth: Orderbook depth

        Returns:
            Orderbook snapshot
        """
        params = {}
        if outcome:
            params["outcome"] = outcome
        if depth:
            params["depth"] = depth

        data = await self._request(
            "GET", f"/upp/v1/markets/{market_id}/orderbook", params=params
        )
        return OrderbookResponse.model_validate(data)

    async def get_merged_orderbook(
        self,
        market_id: str,
        outcome: Optional[str] = None,
        depth: Optional[int] = None,
    ) -> MergedOrderbookResponse:
        """
        Get merged order book across providers.

        Args:
            market_id: Market ID
            outcome: Filter by outcome
            depth: Orderbook depth

        Returns:
            Merged orderbook
        """
        params = {}
        if outcome:
            params["outcome"] = outcome
        if depth:
            params["depth"] = depth

        data = await self._request(
            "GET",
            f"/upp/v1/markets/{market_id}/orderbook/merged",
            params=params,
        )
        return MergedOrderbookResponse.model_validate(data)

    async def list_categories(self) -> Dict[str, Any]:
        """List available market categories."""
        return await self._request("GET", "/upp/v1/markets/categories")

    async def get_resolution(self, market_id: str) -> Dict[str, Any]:
        """Get resolution information for a market."""
        return await self._request(
            "GET", f"/upp/v1/resolutions/{market_id}"
        )

    async def list_resolutions(self) -> Dict[str, Any]:
        """List all market resolutions."""
        return await self._request("GET", "/upp/v1/resolutions")

    async def list_settlement_instruments(self) -> Dict[str, Any]:
        """List settlement instruments."""
        return await self._request("GET", "/upp/v1/settlement/instruments")

    async def list_settlement_handlers(self) -> Dict[str, Any]:
        """List settlement handlers."""
        return await self._request("GET", "/upp/v1/settlement/handlers")

    # ─── Trading (Protected) ─────────────────────────────────

    async def create_order(
        self, request: CreateOrderRequest
    ) -> Order:
        """
        Create a new order.

        Args:
            request: Order creation request

        Returns:
            Created order
        """
        data = await self._request(
            "POST",
            "/upp/v1/orders",
            json=request.model_dump(),
            auth=True,
        )
        return Order.model_validate(data)

    async def list_orders(
        self,
        provider: Optional[str] = None,
        market_id: Optional[str] = None,
        status: Optional[OrderStatus] = None,
        limit: Optional[int] = None,
        cursor: Optional[str] = None,
    ) -> OrdersResponse:
        """
        List all orders for the authenticated user.

        Args:
            provider: Filter by provider
            market_id: Filter by market
            status: Filter by status
            limit: Page limit
            cursor: Pagination cursor

        Returns:
            Orders list
        """
        params = {
            k: v
            for k, v in {
                "provider": provider,
                "market_id": market_id,
                "status": status,
                "limit": limit,
                "cursor": cursor,
            }.items()
            if v is not None
        }

        data = await self._request(
            "GET", "/upp/v1/orders", params=params, auth=True
        )
        return OrdersResponse.model_validate(data)

    async def get_order(
        self, order_id: str, provider: Optional[str] = None
    ) -> Order:
        """
        Get a specific order by ID.

        Args:
            order_id: Order ID
            provider: Optional provider ID

        Returns:
            Order data
        """
        params = {}
        if provider:
            params["provider"] = provider

        data = await self._request(
            "GET", f"/upp/v1/orders/{order_id}", params=params, auth=True
        )
        return Order.model_validate(data)

    async def cancel_order(
        self, order_id: str, provider: Optional[str] = None
    ) -> Order:
        """
        Cancel a specific order.

        Args:
            order_id: Order ID
            provider: Optional provider ID

        Returns:
            Cancelled order
        """
        params = {}
        if provider:
            params["provider"] = provider

        data = await self._request(
            "DELETE",
            f"/upp/v1/orders/{order_id}",
            params=params,
            auth=True,
        )
        return Order.model_validate(data)

    async def cancel_all_orders(
        self, provider: str, market_id: Optional[str] = None
    ) -> CancelAllOrdersResponse:
        """
        Cancel all orders for a provider.

        Args:
            provider: Provider ID
            market_id: Optional market ID to limit cancellation

        Returns:
            Cancelled orders
        """
        body = {"provider": provider}
        if market_id:
            body["market_id"] = market_id

        data = await self._request(
            "POST", "/upp/v1/orders/cancel-all", json=body, auth=True
        )
        return CancelAllOrdersResponse.model_validate(data)

    async def estimate_order(
        self, request: EstimateOrderRequest
    ) -> OrderEstimate:
        """
        Estimate order cost and fees.

        Args:
            request: Order estimation request

        Returns:
            Order estimate
        """
        data = await self._request(
            "POST",
            "/upp/v1/orders/estimate",
            json=request.model_dump(),
            auth=True,
        )
        return OrderEstimate.model_validate(data)

    async def list_trades(
        self,
        provider: Optional[str] = None,
        market_id: Optional[str] = None,
        limit: Optional[int] = None,
        cursor: Optional[str] = None,
    ) -> TradesResponse:
        """
        List all trades for the authenticated user.

        Args:
            provider: Filter by provider
            market_id: Filter by market
            limit: Page limit
            cursor: Pagination cursor

        Returns:
            Trades list
        """
        params = {
            k: v
            for k, v in {
                "provider": provider,
                "market_id": market_id,
                "limit": limit,
                "cursor": cursor,
            }.items()
            if v is not None
        }

        data = await self._request(
            "GET", "/upp/v1/trades", params=params, auth=True
        )
        return TradesResponse.model_validate(data)

    # ─── Portfolio (Protected) ───────────────────────────────

    async def list_positions(
        self, provider: Optional[str] = None
    ) -> PositionsResponse:
        """
        Get all positions for the authenticated user.

        Args:
            provider: Optional provider filter

        Returns:
            Positions list
        """
        params = {}
        if provider:
            params["provider"] = provider

        data = await self._request(
            "GET", "/upp/v1/portfolio/positions", params=params, auth=True
        )
        return PositionsResponse.model_validate(data)

    async def get_portfolio_summary(
        self, provider: Optional[str] = None
    ) -> PortfolioSummary:
        """
        Get portfolio summary (total value, PnL, etc.).

        Args:
            provider: Optional provider filter

        Returns:
            Portfolio summary
        """
        params = {}
        if provider:
            params["provider"] = provider

        data = await self._request(
            "GET", "/upp/v1/portfolio/summary", params=params, auth=True
        )
        return PortfolioSummary.model_validate(data)

    async def list_portfolio_balances(
        self, provider: Optional[str] = None
    ) -> PortfolioBalancesResponse:
        """
        Get portfolio balances across currencies.

        Args:
            provider: Optional provider filter

        Returns:
            Portfolio balances
        """
        params = {}
        if provider:
            params["provider"] = provider

        data = await self._request(
            "GET", "/upp/v1/portfolio/balances", params=params, auth=True
        )
        return PortfolioBalancesResponse.model_validate(data)

    # ─── MCP (Model Context Protocol) ────────────────────────

    async def list_mcp_tools(self) -> McpToolsResponse:
        """List all available MCP tools."""
        data = await self._request("GET", "/upp/v1/mcp/tools")
        return McpToolsResponse.model_validate(data)

    async def get_mcp_schema(self) -> McpSchemaResponse:
        """Get MCP schema for all tools."""
        data = await self._request("GET", "/upp/v1/mcp/schema")
        return McpSchemaResponse.model_validate(data)

    async def execute_mcp_tool(
        self, tool: str, params: Dict[str, Any]
    ) -> McpExecuteResponse:
        """
        Execute an MCP tool.

        Args:
            tool: Tool name
            params: Tool parameters

        Returns:
            Tool result
        """
        data = await self._request(
            "POST",
            "/upp/v1/mcp/execute",
            json={"tool": tool, "params": params},
        )
        return McpExecuteResponse.model_validate(data)

    async def get_agent_card(self) -> Dict[str, Any]:
        """Get the agent card (A2A integration)."""
        return await self._request("GET", "/.well-known/agent.json")

    async def close(self) -> None:
        """Close the client connection."""
        await self._client.aclose()

    async def __aenter__(self) -> "UppClient":
        """Async context manager entry."""
        return self

    async def __aexit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        """Async context manager exit."""
        await self.close()
