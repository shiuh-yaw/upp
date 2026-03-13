"""
UPP SDK for Python

Auto-generated client library for the Universal Prediction Protocol gateway.
Provides typed access to all REST API endpoints, WebSocket subscriptions, and MCP tools.
"""

from .client import UppApiError, UppClient
from .mcp import AgentCardProvider, McpHelper
from .types import *
from .websocket import UppWebSocket

__version__ = "1.0.0"

__all__ = [
    # Version
    "__version__",
    # Client
    "UppClient",
    "UppApiError",
    # WebSocket
    "UppWebSocket",
    # MCP
    "McpHelper",
    "AgentCardProvider",
    # Types
    "UniversalMarketId",
    "Market",
    "Event",
    "Outcome",
    "MarketType",
    "MarketPricing",
    "MarketVolume",
    "MarketLifecycle",
    "MarketStatus",
    "MarketRules",
    "MarketRegulatory",
    "KycLevel",
    "Order",
    "OrderFees",
    "Side",
    "OrderType",
    "TimeInForce",
    "OrderStatus",
    "Position",
    "PositionStatus",
    "Trade",
    "TradeRole",
    "PaginationRequest",
    "PaginationResponse",
    "UppError",
    "UppErrorDetail",
    "ProviderManifest",
    "WellKnown",
    "HealthStatus",
    "MarketsResponse",
    "OrdersResponse",
    "TradesResponse",
    "PositionsResponse",
    "PortfolioSummary",
    "PortfolioBalance",
    "PortfolioBalancesResponse",
    "OrderbookSnapshot",
    "OrderbookResponse",
    "MergedOrderbookResponse",
    "CreateOrderRequest",
    "EstimateOrderRequest",
    "OrderEstimate",
    "CancelAllOrdersRequest",
    "CancelAllOrdersResponse",
    "McpTool",
    "McpToolsResponse",
    "McpExecuteRequest",
    "McpExecuteResponse",
    "McpSchemaResponse",
    "JsonRpcRequest",
    "JsonRpcResponse",
    "PriceSubscription",
    "OrderbookSubscription",
    "PriceUpdate",
    "OrderbookUpdate",
]
