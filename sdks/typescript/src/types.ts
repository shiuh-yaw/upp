/**
 * UPP SDK TypeScript Types
 *
 * Auto-generated type definitions matching the Rust gateway types.
 * These types represent the complete UPP data model for prediction markets.
 */

// ─── Universal Market ID ─────────────────────────────────────

/**
 * Universal Market Identifier in format "upp:{provider}:{native_id}"
 * Uniquely identifies a market across all providers.
 */
export interface UniversalMarketId {
  provider: string;
  native_id: string;
}

// ─── Market ──────────────────────────────────────────────────

/**
 * Complete market data including pricing, volume, and lifecycle state.
 */
export interface Market {
  id: UniversalMarketId;
  event: Event;
  market_type: MarketType;
  outcomes: Outcome[];
  pricing: MarketPricing;
  volume: MarketVolume;
  lifecycle: MarketLifecycle;
  rules: MarketRules;
  regulatory: MarketRegulatory;
  provider_metadata: Record<string, string>;
}

/**
 * Event metadata associated with a market.
 */
export interface Event {
  id: string;
  title: string;
  description: string;
  category: string;
  tags: string[];
  image_url?: string;
  series_id?: string;
  series_title?: string;
}

/**
 * Possible market outcome.
 */
export interface Outcome {
  id: string;
  label: string;
  token_id?: string;
}

/**
 * Market type enumeration.
 */
export type MarketType = 'binary' | 'categorical' | 'scalar';

// ─── Pricing ─────────────────────────────────────────────────

/**
 * Real-time pricing information for a market.
 */
export interface MarketPricing {
  last_price: Record<string, string>;
  best_bid: Record<string, string>;
  best_ask: Record<string, string>;
  mid_price: Record<string, string>;
  spread: Record<string, string>;
  tick_size: string;
  currency: string;
  min_order_size: number;
  max_order_size: number;
  updated_at: string; // ISO 8601 datetime
}

/**
 * Volume and liquidity metrics.
 */
export interface MarketVolume {
  total_volume: string;
  volume_24h: string;
  volume_7d?: string;
  open_interest: string;
  num_traders?: number;
  updated_at: string; // ISO 8601 datetime
}

// ─── Market Lifecycle ────────────────────────────────────────

/**
 * Market lifecycle and status information.
 */
export interface MarketLifecycle {
  status: MarketStatus;
  created_at: string; // ISO 8601 datetime
  opens_at?: string;
  closes_at?: string;
  resolved_at?: string;
  expires_at?: string;
  resolution_source?: string;
}

/**
 * Market status enumeration.
 */
export type MarketStatus = 'pending' | 'open' | 'halted' | 'closed' | 'resolved' | 'disputed' | 'voided';

/**
 * Market trading rules and constraints.
 */
export interface MarketRules {
  allowed_order_types: OrderType[];
  allowed_tif: TimeInForce[];
  allows_short_selling: boolean;
  allows_partial_fill: boolean;
  maker_fee_rate: string;
  taker_fee_rate: string;
  max_position_size: number;
}

/**
 * Market regulatory and compliance information.
 */
export interface MarketRegulatory {
  jurisdiction: string;
  compliant: boolean;
  eligible_regions: string[];
  restricted_regions: string[];
  regulator: string;
  license_type: string;
  contract_type: string;
  required_kyc: KycLevel;
}

/**
 * KYC requirement level.
 */
export type KycLevel = 'none' | 'basic' | 'enhanced' | 'institutional';

// ─── Order ───────────────────────────────────────────────────

/**
 * Complete order information.
 */
export interface Order {
  id: string;
  provider_order_id: string;
  client_order_id?: string;
  market_id: UniversalMarketId;
  outcome_id: string;
  side: Side;
  order_type: OrderType;
  tif: TimeInForce;
  price?: string;
  quantity: number;
  filled_quantity: number;
  average_fill_price?: string;
  remaining_quantity: number;
  status: OrderStatus;
  fees: OrderFees;
  cost_basis?: string;
  created_at: string; // ISO 8601 datetime
  updated_at: string;
  expires_at?: string;
  filled_at?: string;
  cancelled_at?: string;
  cancel_reason?: string;
}

/**
 * Order side enumeration.
 */
export type Side = 'buy' | 'sell';

/**
 * Order type enumeration.
 */
export type OrderType = 'limit' | 'market';

/**
 * Time in force enumeration.
 */
export type TimeInForce = 'GTC' | 'GTD' | 'FOK' | 'IOC';

/**
 * Order status enumeration.
 */
export type OrderStatus = 'pending' | 'open' | 'partially_filled' | 'filled' | 'cancelled' | 'rejected' | 'expired';

/**
 * Order fees breakdown.
 */
export interface OrderFees {
  maker_fee: string;
  taker_fee: string;
  total_fee: string;
}

// ─── Position ────────────────────────────────────────────────

/**
 * Active position in a market outcome.
 */
export interface Position {
  market_id: UniversalMarketId;
  outcome_id: string;
  quantity: number;
  average_entry_price: string;
  current_price: string;
  cost_basis: string;
  current_value: string;
  unrealized_pnl: string;
  realized_pnl: string;
  status: PositionStatus;
  opened_at: string; // ISO 8601 datetime
  updated_at: string;
  market_title: string;
  market_status: MarketStatus;
}

/**
 * Position status enumeration.
 */
export type PositionStatus = 'open' | 'closed' | 'settled' | 'expired';

// ─── Trade ───────────────────────────────────────────────────

/**
 * Executed trade.
 */
export interface Trade {
  id: string;
  order_id: string;
  market_id: UniversalMarketId;
  outcome_id: string;
  side: Side;
  price: string;
  quantity: number;
  notional: string;
  role: TradeRole;
  fees: OrderFees;
  executed_at: string; // ISO 8601 datetime
}

/**
 * Trade role enumeration.
 */
export type TradeRole = 'maker' | 'taker';

// ─── Pagination ──────────────────────────────────────────────

/**
 * Pagination request parameters.
 */
export interface PaginationRequest {
  limit?: number;
  cursor?: string;
}

/**
 * Pagination response metadata.
 */
export interface PaginationResponse {
  cursor: string;
  has_more: boolean;
  total: number;
}

// ─── Error ───────────────────────────────────────────────────

/**
 * Error response from the API.
 */
export interface UppError {
  error: {
    code: string;
    message: string;
    request_id: string;
    provider?: string;
    details?: Record<string, string>;
  };
}

// ─── Discovery ───────────────────────────────────────────────

/**
 * Provider manifest information.
 */
export interface ProviderManifest {
  id: string;
  name: string;
  version: string;
  capabilities: string[];
  authentication: string[];
  protocols: string[];
  [key: string]: unknown;
}

/**
 * Well-known endpoint response.
 */
export interface WellKnown {
  upp_version: string;
  gateway: {
    version: string;
    transports: string[];
  };
  providers: ProviderManifest[];
}

/**
 * Health check response.
 */
export interface HealthStatus {
  provider: string;
  status: 'healthy' | 'degraded' | 'down';
  response_time_ms: number;
  last_check: string;
  [key: string]: unknown;
}

// ─── List Response Wrappers ──────────────────────────────────

/**
 * Markets list response.
 */
export interface MarketsResponse {
  markets: Market[];
  pagination: PaginationResponse;
  provider_results?: Record<string, unknown>;
  errors?: Record<string, string>;
}

/**
 * Orders list response.
 */
export interface OrdersResponse {
  orders: Order[];
  pagination: PaginationResponse;
}

/**
 * Trades list response.
 */
export interface TradesResponse {
  trades: Trade[];
  pagination: PaginationResponse;
}

/**
 * Positions list response.
 */
export interface PositionsResponse {
  positions: Position[];
  total: number;
}

/**
 * Portfolio summary response.
 */
export interface PortfolioSummary {
  total_value: string;
  total_pnl: string;
  total_pnl_percent: string;
  position_count: number;
  provider_summaries: Array<{
    provider: string;
    total_value: string;
    total_pnl: string;
  }>;
}

/**
 * Portfolio balances response.
 */
export interface PortfolioBalance {
  provider: string;
  balances: Array<{
    currency: string;
    amount: string;
  }>;
}

// ─── Orderbook ───────────────────────────────────────────────

/**
 * Order book snapshot.
 */
export interface OrderbookSnapshot {
  outcome_id: string;
  bids: Array<[string, string]>; // [price, quantity]
  asks: Array<[string, string]>;
  mid_price: string;
  spread: string;
  timestamp: string;
}

/**
 * Orderbook response.
 */
export interface OrderbookResponse {
  market_id: string;
  orderbook: OrderbookSnapshot[];
}

/**
 * Merged orderbook response (cross-provider).
 */
export interface MergedOrderbookResponse {
  market_id: string;
  orderbook: OrderbookSnapshot[];
  providers: string[];
}

// ─── Trading Requests ────────────────────────────────────────

/**
 * Request to create a new order.
 */
export interface CreateOrderRequest {
  provider: string;
  market_id: string;
  outcome_id: string;
  side: Side;
  order_type: OrderType;
  tif?: TimeInForce;
  price?: string;
  quantity: number;
  client_order_id?: string;
}

/**
 * Request to estimate order cost.
 */
export interface EstimateOrderRequest {
  provider: string;
  market_id: string;
  outcome_id: string;
  side: Side;
  price: string;
  quantity: number;
}

/**
 * Order estimate response.
 */
export interface OrderEstimate {
  provider: string;
  market_id: string;
  outcome_id: string;
  side: Side;
  estimated_cost: string;
  estimated_fee: string;
  estimated_total: string;
  price: string;
  quantity: number;
}

/**
 * Request to cancel all orders.
 */
export interface CancelAllOrdersRequest {
  provider: string;
  market_id?: string;
}

/**
 * Cancel all orders response.
 */
export interface CancelAllOrdersResponse {
  cancelled: Order[];
  count: number;
}

// ─── MCP (Model Context Protocol) ────────────────────────────

/**
 * MCP tool definition.
 */
export interface McpTool {
  name: string;
  description: string;
  input_schema: Record<string, unknown>;
}

/**
 * MCP tools list response.
 */
export interface McpToolsResponse {
  tools: McpTool[];
  total: number;
  mcp_version: string;
}

/**
 * MCP tool execution request.
 */
export interface McpExecuteRequest {
  tool: string;
  params: Record<string, unknown>;
}

/**
 * MCP tool execution response.
 */
export interface McpExecuteResponse {
  tool: string;
  result: unknown;
  status: 'ok' | 'error';
}

/**
 * MCP schema response.
 */
export interface McpSchemaResponse {
  openapi: string;
  info: {
    title: string;
    description: string;
    version: string;
  };
  servers: Array<{
    url: string;
    description: string;
  }>;
  'x-mcp-tools': McpTool[];
  components: {
    schemas: Record<string, unknown>;
  };
}

/**
 * Agent card (A2A integration).
 */
export interface AgentCard {
  [key: string]: unknown;
}

// ─── WebSocket ───────────────────────────────────────────────

/**
 * WebSocket JSON-RPC request.
 */
export interface JsonRpcRequest {
  jsonrpc: '2.0';
  id: string | number;
  method: string;
  params?: Record<string, unknown>;
}

/**
 * WebSocket JSON-RPC response.
 */
export interface JsonRpcResponse {
  jsonrpc: '2.0';
  id: string | number;
  result?: unknown;
  error?: {
    code: number;
    message: string;
    data?: unknown;
  };
}

/**
 * Price subscription message.
 */
export interface PriceSubscription {
  channel: 'prices';
  market_ids: string[];
  interval_ms: number;
}

/**
 * Orderbook subscription message.
 */
export interface OrderbookSubscription {
  channel: 'orderbook';
  market_ids: string[];
  depth: number;
  interval_ms: number;
}

/**
 * Price update (fan-out message).
 */
export interface PriceUpdate {
  channel: 'prices';
  market_id: string;
  timestamp: string;
  prices: Record<string, string>;
}

/**
 * Orderbook update (fan-out message).
 */
export interface OrderbookUpdate {
  channel: 'orderbook';
  market_id: string;
  timestamp: string;
  snapshots: OrderbookSnapshot[];
}

/**
 * WebSocket fan-out message.
 */
export type FanOutMessage = PriceUpdate | OrderbookUpdate;
