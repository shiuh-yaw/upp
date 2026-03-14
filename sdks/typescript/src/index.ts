/**
 * UPP TypeScript SDK Client
 *
 * A type-safe client for the Universal Prediction Protocol API.
 * Provides unified access to prediction market aggregation, trading,
 * arbitrage detection, and real-time data streaming.
 */

export class UppError extends Error {
  constructor(
    public code: string,
    public message: string,
    public requestId?: string,
  ) {
    super(message);
    this.name = 'UppError';
    Object.setPrototypeOf(this, UppError.prototype);
  }
}

/**
 * Market outcome definition
 */
export interface Outcome {
  id: string;
  name: string;
  price: number;
}

/**
 * Market details with current state
 */
export interface Market {
  id: string;
  name: string;
  provider: string;
  status: 'open' | 'closed' | 'settled';
  price: number;
  volume: number;
  outcomes: Outcome[];
  created_at: string;
}

/**
 * Orderbook level (bid/ask)
 */
export interface OrderbookLevel {
  price: number;
  quantity: number;
}

/**
 * Full orderbook with bids and asks
 */
export interface Orderbook {
  market_id: string;
  bids: OrderbookLevel[];
  asks: OrderbookLevel[];
  timestamp?: string;
}

/**
 * Candlestick OHLCV data
 */
export interface Candle {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

/**
 * Order side (buy/sell)
 */
export type OrderSide = 'buy' | 'sell';

/**
 * Order type (limit/market)
 */
export type OrderType = 'limit' | 'market';

/**
 * Request to create a new order
 */
export interface CreateOrderRequest {
  market_id: string;
  side: OrderSide;
  price: number;
  quantity: number;
  order_type?: OrderType;
}

/**
 * Order details
 */
export interface Order {
  id: string;
  market_id: string;
  side: OrderSide;
  price: number;
  quantity: number;
  filled: number;
  status: string;
  created_at: string;
  updated_at?: string;
}

/**
 * Open position
 */
export interface Position {
  market_id: string;
  outcome_id: string;
  quantity: number;
  entry_price: number;
  current_price: number;
  pnl: number;
  pnl_percent: number;
}

/**
 * Portfolio summary with totals
 */
export interface PortfolioSummary {
  total_balance: number;
  available_balance: number;
  positions: Position[];
  total_pnl: number;
  win_rate: number;
  sharpe_ratio: number;
}

/**
 * Arbitrage opportunity across providers
 */
export interface ArbitrageOpportunity {
  id: string;
  market_id: string;
  spread_percent: number;
  buy_provider: string;
  buy_price: number;
  sell_provider: string;
  sell_price: number;
  estimated_profit: number;
  timestamp: string;
}

/**
 * Health check response
 */
export interface HealthResponse {
  status: string;
  version: string;
  uptime: string;
}

/**
 * Provider information
 */
export interface Provider {
  id: string;
  name: string;
  status: string;
  markets_count?: number;
}

/**
 * List response with pagination
 */
export interface ListResponse<T> {
  data: T[];
  cursor?: string;
  limit?: number;
  total?: number;
}

/**
 * Search result for market search
 */
export interface SearchResult {
  market: Market;
  relevance_score?: number;
}

/**
 * Main UPP API Client
 *
 * Usage:
 * ```typescript
 * const client = new UppClient({
 *   baseUrl: 'https://api.upp.dev',
 *   apiKey: 'your-api-key'
 * });
 *
 * const health = await client.health();
 * const markets = await client.listMarkets();
 * ```
 */
export class UppClient {
  private baseUrl: string;
  private apiKey?: string;
  private defaultHeaders: Record<string, string>;

  constructor(config: { baseUrl: string; apiKey?: string }) {
    this.baseUrl = config.baseUrl.replace(/\/$/, '');
    this.apiKey = config.apiKey;
    this.defaultHeaders = {
      'Content-Type': 'application/json',
    };
    if (this.apiKey) {
      this.defaultHeaders['X-API-Key'] = this.apiKey;
    }
  }

  private async fetch<T>(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const options: RequestInit = {
      method,
      headers: this.defaultHeaders,
    };

    if (body) {
      options.body = JSON.stringify(body);
    }

    const response = await fetch(url, options);
    const data = await response.json();

    if (!response.ok) {
      const error = data.error || {};
      throw new UppError(
        error.code || 'HTTP_ERROR',
        error.message || `HTTP ${response.status}`,
        error.request_id,
      );
    }

    return data;
  }

  async health(): Promise<HealthResponse> {
    return this.fetch('GET', '/health');
  }

  async healthProvider(provider: string): Promise<unknown> {
    return this.fetch('GET', `/upp/v1/discovery/health/${provider}`);
  }

  async listProviders(): Promise<ListResponse<Provider>> {
    return this.fetch('GET', '/upp/v1/discovery/providers');
  }

  async getProviderManifest(provider: string): Promise<unknown> {
    return this.fetch('GET', `/upp/v1/discovery/manifest/${provider}`);
  }

  async listMarkets(params?: {
    provider?: string;
    status?: 'open' | 'closed' | 'settled';
    category?: string;
    limit?: number;
    cursor?: string;
  }): Promise<ListResponse<Market>> {
    const query = new URLSearchParams();
    if (params) {
      if (params.provider) query.append('provider', params.provider);
      if (params.status) query.append('status', params.status);
      if (params.category) query.append('category', params.category);
      if (params.limit) query.append('limit', params.limit.toString());
      if (params.cursor) query.append('cursor', params.cursor);
    }
    const path = `/upp/v1/markets${query.toString() ? '?' + query : ''}`;
    return this.fetch('GET', path);
  }

  async searchMarkets(
    query: string,
    provider?: string,
    limit: number = 20,
  ): Promise<ListResponse<SearchResult>> {
    const params = new URLSearchParams({
      q: query,
      limit: limit.toString(),
    });
    if (provider) params.append('provider', provider);
    return this.fetch('GET', `/upp/v1/markets/search?${params}`);
  }

  async getMarket(marketId: string): Promise<Market> {
    return this.fetch('GET', `/upp/v1/markets/${marketId}`);
  }

  async getOrderbook(marketId: string): Promise<Orderbook> {
    return this.fetch('GET', `/upp/v1/markets/${marketId}/orderbook`);
  }

  async getMergedOrderbook(marketId: string): Promise<Orderbook> {
    return this.fetch('GET', `/upp/v1/markets/${marketId}/orderbook/merged`);
  }

  async listCategories(): Promise<ListResponse<{ id: string; name: string }>> {
    return this.fetch('GET', '/upp/v1/markets/categories');
  }

  async listArbitrage(): Promise<ListResponse<ArbitrageOpportunity>> {
    return this.fetch('GET', '/upp/v1/arbitrage');
  }

  async arbitrageSummary(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/arbitrage/summary');
  }

  async arbitrageHistory(limit: number = 100): Promise<unknown> {
    return this.fetch('GET', `/upp/v1/arbitrage/history?limit=${limit}`);
  }

  async getCandles(
    marketId: string,
    options?: {
      outcome?: string;
      resolution?: '1m' | '5m' | '1h' | '1d';
      limit?: number;
      start?: number;
      end?: number;
    },
  ): Promise<Candle[]> {
    const query = new URLSearchParams();
    if (options) {
      if (options.outcome) query.append('outcome', options.outcome);
      if (options.resolution) query.append('resolution', options.resolution);
      if (options.limit) query.append('limit', options.limit.toString());
      if (options.start) query.append('start', options.start.toString());
      if (options.end) query.append('end', options.end.toString());
    }
    const path = `/upp/v1/markets/${marketId}/candles${query.toString() ? '?' + query : ''}`;
    return this.fetch('GET', path);
  }

  async getLatestCandle(
    marketId: string,
    outcome?: string,
    resolution: '1m' | '5m' | '1h' | '1d' = '1h',
  ): Promise<Candle> {
    const query = new URLSearchParams({ resolution });
    if (outcome) query.append('outcome', outcome);
    return this.fetch('GET', `/upp/v1/markets/${marketId}/candles/latest?${query}`);
  }

  async priceIndexStats(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/price-index/stats');
  }

  async listResolutions(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/resolutions');
  }

  async getResolution(marketId: string): Promise<unknown> {
    return this.fetch('GET', `/upp/v1/resolutions/${marketId}`);
  }

  async createOrder(order: CreateOrderRequest): Promise<Order> {
    return this.fetch('POST', '/upp/v1/orders', order);
  }

  async listOrders(params?: {
    provider?: string;
    status?: string;
    limit?: number;
  }): Promise<ListResponse<Order>> {
    const query = new URLSearchParams();
    if (params) {
      if (params.provider) query.append('provider', params.provider);
      if (params.status) query.append('status', params.status);
      if (params.limit) query.append('limit', params.limit.toString());
    }
    const path = `/upp/v1/orders${query.toString() ? '?' + query : ''}`;
    return this.fetch('GET', path);
  }

  async getOrder(orderId: string): Promise<Order> {
    return this.fetch('GET', `/upp/v1/orders/${orderId}`);
  }

  async cancelOrder(orderId: string): Promise<{ status: string }> {
    return this.fetch('DELETE', `/upp/v1/orders/${orderId}`);
  }

  async cancelAllOrders(): Promise<{ cancelled: number }> {
    return this.fetch('POST', '/upp/v1/orders/cancel-all');
  }

  async estimateOrder(order: CreateOrderRequest): Promise<unknown> {
    return this.fetch('POST', '/upp/v1/orders/estimate', order);
  }

  async listTrades(limit: number = 50): Promise<ListResponse<unknown>> {
    return this.fetch('GET', `/upp/v1/trades?limit=${limit}`);
  }

  async listPositions(): Promise<ListResponse<Position>> {
    return this.fetch('GET', '/upp/v1/portfolio/positions');
  }

  async getPortfolioSummary(): Promise<PortfolioSummary> {
    return this.fetch('GET', '/upp/v1/portfolio/summary');
  }

  async listBalances(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/portfolio/balances');
  }

  async getPortfolioAnalytics(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/portfolio/analytics');
  }

  async feedStatus(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/feeds/status');
  }

  async feedStats(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/feeds/stats');
  }

  async subscribeFeed(providerId: string, marketIds: string[]): Promise<unknown> {
    return this.fetch('POST', '/upp/v1/feeds/subscribe', {
      provider_id: providerId,
      market_ids: marketIds,
    });
  }

  async listStrategies(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/backtest/strategies');
  }

  async runBacktest(params: {
    strategy: string;
    market_id: string;
    params?: Record<string, unknown>;
  }): Promise<unknown> {
    return this.fetch('POST', '/upp/v1/backtest/run', params);
  }

  async compareStrategies(params?: unknown): Promise<unknown> {
    return this.fetch('POST', '/upp/v1/backtest/compare', params);
  }

  async ingestionStats(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/ingestion/stats');
  }

  async listInstruments(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/settlement/instruments');
  }

  async listHandlers(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/settlement/handlers');
  }

  async listMcpTools(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/mcp/tools');
  }

  async executeMcpTool(tool: string, params?: Record<string, unknown>): Promise<unknown> {
    return this.fetch('POST', '/upp/v1/mcp/execute', { tool, params });
  }

  async mcpSchema(): Promise<unknown> {
    return this.fetch('GET', '/upp/v1/mcp/schema');
  }

  async ready(): Promise<unknown> {
    return this.fetch('GET', '/ready');
  }

  async metrics(): Promise<string> {
    const url = `${this.baseUrl}/metrics`;
    const response = await fetch(url);
    if (!response.ok) {
      throw new UppError('HTTP_ERROR', `HTTP ${response.status}`);
    }
    return response.text();
  }
}
