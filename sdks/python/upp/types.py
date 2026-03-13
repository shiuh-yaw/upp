"""
UPP SDK Python Types

Auto-generated type definitions matching the Rust gateway types using Pydantic.
These models represent the complete UPP data model for prediction markets.
"""

from datetime import datetime
from enum import Enum
from typing import Any, Dict, List, Optional

from pydantic import BaseModel, Field


# ─── Universal Market ID ─────────────────────────────────────


class UniversalMarketId(BaseModel):
    """Universal Market Identifier in format 'upp:{provider}:{native_id}'"""

    provider: str
    native_id: str

    def to_full_id(self) -> str:
        """Format as 'upp:{provider}:{native_id}'"""
        return f"upp:{self.provider}:{self.native_id}"

    @staticmethod
    def parse(full_id: str) -> Optional["UniversalMarketId"]:
        """Parse from 'upp:{provider}:{native_id}' format"""
        parts = full_id.split(":", 2)
        if len(parts) == 3 and parts[0] == "upp":
            return UniversalMarketId(provider=parts[1], native_id=parts[2])
        return None


# ─── Enumerations ───────────────────────────────────────────


class MarketType(str, Enum):
    """Market type enumeration."""

    BINARY = "binary"
    CATEGORICAL = "categorical"
    SCALAR = "scalar"


class MarketStatus(str, Enum):
    """Market status enumeration."""

    PENDING = "pending"
    OPEN = "open"
    HALTED = "halted"
    CLOSED = "closed"
    RESOLVED = "resolved"
    DISPUTED = "disputed"
    VOIDED = "voided"


class Side(str, Enum):
    """Order side enumeration."""

    BUY = "buy"
    SELL = "sell"


class OrderType(str, Enum):
    """Order type enumeration."""

    LIMIT = "limit"
    MARKET = "market"


class TimeInForce(str, Enum):
    """Time in force enumeration."""

    GTC = "GTC"
    GTD = "GTD"
    FOK = "FOK"
    IOC = "IOC"


class OrderStatus(str, Enum):
    """Order status enumeration."""

    PENDING = "pending"
    OPEN = "open"
    PARTIALLY_FILLED = "partially_filled"
    FILLED = "filled"
    CANCELLED = "cancelled"
    REJECTED = "rejected"
    EXPIRED = "expired"


class PositionStatus(str, Enum):
    """Position status enumeration."""

    OPEN = "open"
    CLOSED = "closed"
    SETTLED = "settled"
    EXPIRED = "expired"


class KycLevel(str, Enum):
    """KYC requirement level."""

    NONE = "none"
    BASIC = "basic"
    ENHANCED = "enhanced"
    INSTITUTIONAL = "institutional"


class TradeRole(str, Enum):
    """Trade role enumeration."""

    MAKER = "maker"
    TAKER = "taker"


# ─── Market ──────────────────────────────────────────────────


class Event(BaseModel):
    """Event metadata associated with a market."""

    id: str
    title: str
    description: str
    category: str
    tags: List[str] = Field(default_factory=list)
    image_url: Optional[str] = None
    series_id: Optional[str] = None
    series_title: Optional[str] = None


class Outcome(BaseModel):
    """Possible market outcome."""

    id: str
    label: str
    token_id: Optional[str] = None


class MarketPricing(BaseModel):
    """Real-time pricing information for a market."""

    last_price: Dict[str, str]
    best_bid: Dict[str, str]
    best_ask: Dict[str, str]
    mid_price: Dict[str, str]
    spread: Dict[str, str]
    tick_size: str
    currency: str
    min_order_size: int
    max_order_size: int
    updated_at: datetime


class MarketVolume(BaseModel):
    """Volume and liquidity metrics."""

    total_volume: str
    volume_24h: str
    volume_7d: Optional[str] = None
    open_interest: str
    num_traders: Optional[int] = None
    updated_at: datetime


class MarketLifecycle(BaseModel):
    """Market lifecycle and status information."""

    status: MarketStatus
    created_at: datetime
    opens_at: Optional[datetime] = None
    closes_at: Optional[datetime] = None
    resolved_at: Optional[datetime] = None
    expires_at: Optional[datetime] = None
    resolution_source: Optional[str] = None


class MarketRules(BaseModel):
    """Market trading rules and constraints."""

    allowed_order_types: List[OrderType]
    allowed_tif: List[TimeInForce]
    allows_short_selling: bool
    allows_partial_fill: bool
    maker_fee_rate: str
    taker_fee_rate: str
    max_position_size: int


class MarketRegulatory(BaseModel):
    """Market regulatory and compliance information."""

    jurisdiction: str
    compliant: bool
    eligible_regions: List[str]
    restricted_regions: List[str]
    regulator: str
    license_type: str
    contract_type: str
    required_kyc: KycLevel


class Market(BaseModel):
    """Complete market data including pricing, volume, and lifecycle state."""

    id: UniversalMarketId
    event: Event
    market_type: MarketType
    outcomes: List[Outcome]
    pricing: MarketPricing
    volume: MarketVolume
    lifecycle: MarketLifecycle
    rules: MarketRules
    regulatory: MarketRegulatory
    provider_metadata: Dict[str, str] = Field(default_factory=dict)


# ─── Order ───────────────────────────────────────────────────


class OrderFees(BaseModel):
    """Order fees breakdown."""

    maker_fee: str
    taker_fee: str
    total_fee: str


class Order(BaseModel):
    """Complete order information."""

    id: str
    provider_order_id: str
    client_order_id: Optional[str] = None
    market_id: UniversalMarketId
    outcome_id: str
    side: Side
    order_type: OrderType
    tif: TimeInForce
    price: Optional[str] = None
    quantity: int
    filled_quantity: int
    average_fill_price: Optional[str] = None
    remaining_quantity: int
    status: OrderStatus
    fees: OrderFees
    cost_basis: Optional[str] = None
    created_at: datetime
    updated_at: datetime
    expires_at: Optional[datetime] = None
    filled_at: Optional[datetime] = None
    cancelled_at: Optional[datetime] = None
    cancel_reason: Optional[str] = None


# ─── Position ────────────────────────────────────────────────


class Position(BaseModel):
    """Active position in a market outcome."""

    market_id: UniversalMarketId
    outcome_id: str
    quantity: int
    average_entry_price: str
    current_price: str
    cost_basis: str
    current_value: str
    unrealized_pnl: str
    realized_pnl: str
    status: PositionStatus
    opened_at: datetime
    updated_at: datetime
    market_title: str
    market_status: MarketStatus


# ─── Trade ───────────────────────────────────────────────────


class Trade(BaseModel):
    """Executed trade."""

    id: str
    order_id: str
    market_id: UniversalMarketId
    outcome_id: str
    side: Side
    price: str
    quantity: int
    notional: str
    role: TradeRole
    fees: OrderFees
    executed_at: datetime


# ─── Pagination ──────────────────────────────────────────────


class PaginationRequest(BaseModel):
    """Pagination request parameters."""

    limit: Optional[int] = None
    cursor: Optional[str] = None


class PaginationResponse(BaseModel):
    """Pagination response metadata."""

    cursor: str
    has_more: bool
    total: int


# ─── Error ───────────────────────────────────────────────────


class UppErrorDetail(BaseModel):
    """API error response."""

    code: str
    message: str
    request_id: str
    provider: Optional[str] = None
    details: Dict[str, str] = Field(default_factory=dict)


class UppError(BaseModel):
    """Error response wrapper."""

    error: UppErrorDetail


# ─── Discovery ───────────────────────────────────────────────


class ProviderManifest(BaseModel):
    """Provider manifest information."""

    id: str
    name: str
    version: str
    capabilities: List[str]
    authentication: List[str]
    protocols: List[str]

    class Config:
        extra = "allow"  # Allow additional fields


class WellKnown(BaseModel):
    """Well-known endpoint response."""

    upp_version: str
    gateway: Dict[str, Any]
    providers: List[ProviderManifest]


class HealthStatus(BaseModel):
    """Health check response."""

    provider: str
    status: str
    response_time_ms: int
    last_check: str

    class Config:
        extra = "allow"


# ─── List Response Wrappers ──────────────────────────────────


class MarketsResponse(BaseModel):
    """Markets list response."""

    markets: List[Market]
    pagination: PaginationResponse
    provider_results: Optional[Dict[str, Any]] = None
    errors: Optional[Dict[str, str]] = None


class OrdersResponse(BaseModel):
    """Orders list response."""

    orders: List[Order]
    pagination: PaginationResponse


class TradesResponse(BaseModel):
    """Trades list response."""

    trades: List[Trade]
    pagination: PaginationResponse


class PositionsResponse(BaseModel):
    """Positions list response."""

    positions: List[Position]
    total: int


class PortfolioSummary(BaseModel):
    """Portfolio summary response."""

    total_value: str
    total_pnl: str
    total_pnl_percent: str
    position_count: int
    provider_summaries: List[Dict[str, Any]]


class PortfolioBalance(BaseModel):
    """Portfolio balance entry."""

    provider: str
    balances: List[Dict[str, str]]


class PortfolioBalancesResponse(BaseModel):
    """Portfolio balances response."""

    balances: List[PortfolioBalance]
    total: int


# ─── Orderbook ───────────────────────────────────────────────


class OrderbookSnapshot(BaseModel):
    """Order book snapshot."""

    outcome_id: str
    bids: List[List[str]]  # [price, quantity]
    asks: List[List[str]]  # [price, quantity]
    mid_price: str
    spread: str
    timestamp: str


class OrderbookResponse(BaseModel):
    """Orderbook response."""

    market_id: str
    orderbook: List[OrderbookSnapshot]


class MergedOrderbookResponse(BaseModel):
    """Merged orderbook response (cross-provider)."""

    market_id: str
    orderbook: List[OrderbookSnapshot]
    providers: List[str]


# ─── Trading Requests ────────────────────────────────────────


class CreateOrderRequest(BaseModel):
    """Request to create a new order."""

    provider: str
    market_id: str
    outcome_id: str
    side: Side
    order_type: OrderType
    tif: Optional[TimeInForce] = None
    price: Optional[str] = None
    quantity: int
    client_order_id: Optional[str] = None


class EstimateOrderRequest(BaseModel):
    """Request to estimate order cost."""

    provider: str
    market_id: str
    outcome_id: str
    side: Side
    price: str
    quantity: int


class OrderEstimate(BaseModel):
    """Order estimate response."""

    provider: str
    market_id: str
    outcome_id: str
    side: Side
    estimated_cost: str
    estimated_fee: str
    estimated_total: str
    price: str
    quantity: int


class CancelAllOrdersRequest(BaseModel):
    """Request to cancel all orders."""

    provider: str
    market_id: Optional[str] = None


class CancelAllOrdersResponse(BaseModel):
    """Cancel all orders response."""

    cancelled: List[Order]
    count: int


# ─── MCP (Model Context Protocol) ────────────────────────────


class McpTool(BaseModel):
    """MCP tool definition."""

    name: str
    description: str
    input_schema: Dict[str, Any]


class McpToolsResponse(BaseModel):
    """MCP tools list response."""

    tools: List[McpTool]
    total: int
    mcp_version: str


class McpExecuteRequest(BaseModel):
    """MCP tool execution request."""

    tool: str
    params: Dict[str, Any]


class McpExecuteResponse(BaseModel):
    """MCP tool execution response."""

    tool: str
    result: Any
    status: str


class McpSchemaResponse(BaseModel):
    """MCP schema response."""

    openapi: str
    info: Dict[str, str]
    servers: List[Dict[str, str]]
    x_mcp_tools: List[McpTool] = Field(alias="x-mcp-tools")
    components: Dict[str, Any]

    class Config:
        populate_by_name = True


# ─── WebSocket ───────────────────────────────────────────────


class JsonRpcRequest(BaseModel):
    """WebSocket JSON-RPC request."""

    jsonrpc: str = "2.0"
    id: int | str
    method: str
    params: Optional[Dict[str, Any]] = None


class JsonRpcResponse(BaseModel):
    """WebSocket JSON-RPC response."""

    jsonrpc: str = "2.0"
    id: int | str
    result: Optional[Any] = None
    error: Optional[Dict[str, Any]] = None


class PriceSubscription(BaseModel):
    """Price subscription message."""

    channel: str = "prices"
    market_ids: List[str]
    interval_ms: int


class OrderbookSubscription(BaseModel):
    """Orderbook subscription message."""

    channel: str = "orderbook"
    market_ids: List[str]
    depth: int
    interval_ms: int


class PriceUpdate(BaseModel):
    """Price update (fan-out message)."""

    channel: str = "prices"
    market_id: str
    timestamp: str
    prices: Dict[str, str]


class OrderbookUpdate(BaseModel):
    """Orderbook update (fan-out message)."""

    channel: str = "orderbook"
    market_id: str
    timestamp: str
    snapshots: List[OrderbookSnapshot]


# ─── Agent Card ──────────────────────────────────────────────


class AgentCard(BaseModel):
    """Agent card for A2A integration."""

    class Config:
        extra = "allow"  # Allow additional fields
