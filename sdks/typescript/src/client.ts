/**
 * UPP SDK TypeScript Client
 *
 * Main client for interacting with the UPP Gateway REST API.
 * Provides typed methods for all endpoints and automatic error handling.
 */

import * as types from './types';

/**
 * Configuration for the UPP client.
 */
export interface UppClientConfig {
  /** Base URL of the UPP Gateway (e.g., "http://localhost:8080") */
  baseUrl: string;
  /** Optional API key for authenticated requests */
  apiKey?: string;
  /** Request timeout in milliseconds (default: 30000) */
  timeout?: number;
  /** Custom fetch implementation (default: globalThis.fetch) */
  fetch?: typeof fetch;
}

/**
 * Main UPP Client class for REST API interactions.
 *
 * @example
 * ```typescript
 * const client = new UppClient({
 *   baseUrl: 'http://localhost:8080',
 *   apiKey: 'your-api-key'
 * });
 *
 * const market = await client.getMarket('upp:kalshi:MELON-240301');
 * const orders = await client.listOrders();
 * ```
 */
export class UppClient {
  private baseUrl: string;
  private apiKey?: string;
  private timeout: number;
  private fetchImpl: typeof fetch;

  constructor(config: UppClientConfig) {
    this.baseUrl = config.baseUrl.replace(/\/$/, '');
    this.apiKey = config.apiKey;
    this.timeout = config.timeout ?? 30000;
    this.fetchImpl = config.fetch ?? globalThis.fetch;
  }

  /**
   * Make an HTTP request to the UPP Gateway.
   *
   * @internal
   */
  private async request<T>(
    method: string,
    path: string,
    options?: {
      query?: Record<string, string | number | boolean | undefined>;
      body?: unknown;
      auth?: boolean;
    }
  ): Promise<T> {
    const url = new URL(`${this.baseUrl}${path}`);

    if (options?.query) {
      Object.entries(options.query).forEach(([key, value]) => {
        if (value !== undefined) {
          url.searchParams.append(key, String(value));
        }
      });
    }

    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    if (options?.auth && this.apiKey) {
      headers['Authorization'] = `Bearer ${this.apiKey}`;
    }

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeout);

    try {
      const response = await this.fetchImpl(url.toString(), {
        method,
        headers,
        body: options?.body ? JSON.stringify(options.body) : undefined,
        signal: controller.signal,
      });

      if (!response.ok) {
        const error = (await response.json()) as types.UppError;
        throw new UppApiError(
          error.error.message,
          error.error.code,
          response.status,
          error.error.details
        );
      }

      return (await response.json()) as T;
    } finally {
      clearTimeout(timeoutId);
    }
  }

  // ─── Health & Metrics ────────────────────────────────────

  /**
   * Check if the gateway is healthy.
   *
   * @returns Health status
   */
  async health(): Promise<{ status: string; timestamp: string }> {
    return this.request('GET', '/health');
  }

  /**
   * Check if the gateway is ready to serve requests.
   *
   * @returns Readiness status
   */
  async ready(): Promise<{ status: string; timestamp: string }> {
    return this.request('GET', '/ready');
  }

  /**
   * Get metrics from the gateway.
   *
   * @returns Prometheus-style metrics
   */
  async metrics(): Promise<string> {
    const response = await this.fetchImpl(`${this.baseUrl}/metrics`);
    return response.text();
  }

  // ─── Discovery ──────────────────────────────────────────

  /**
   * Get the well-known UPP endpoint.
   *
   * @returns Well-known endpoint information
   */
  async getWellKnown(): Promise<types.WellKnown> {
    return this.request('GET', '/.well-known/upp');
  }

  /**
   * List all available prediction market providers.
   *
   * @returns Provider manifests
   */
  async listProviders(): Promise<{
    providers: types.ProviderManifest[];
    total: number;
  }> {
    return this.request('GET', '/upp/v1/discovery/providers');
  }

  /**
   * Get the manifest for a specific provider.
   *
   * @param provider - Provider ID (e.g., "kalshi.com")
   * @returns Provider manifest
   */
  async getManifest(provider: string): Promise<types.ProviderManifest> {
    return this.request('GET', `/upp/v1/discovery/manifest/${encodeURIComponent(provider)}`);
  }

  /**
   * Negotiate capabilities with a provider.
   *
   * @param provider - Provider ID
   * @returns Negotiated capabilities
   */
  async negotiate(provider: string): Promise<{
    active_capabilities: string[];
    selected_transport: string;
    selected_auth: string;
  }> {
    return this.request('POST', '/upp/v1/discovery/negotiate', {
      body: { provider },
    });
  }

  /**
   * Check health of a specific provider.
   *
   * @param provider - Provider ID
   * @returns Health status
   */
  async checkProviderHealth(provider: string): Promise<types.HealthStatus> {
    return this.request('GET', `/upp/v1/discovery/health/${encodeURIComponent(provider)}`);
  }

  /**
   * Check health of all providers.
   *
   * @returns Health statuses
   */
  async checkAllProviderHealth(): Promise<{
    providers: types.HealthStatus[];
    total: number;
  }> {
    return this.request('GET', '/upp/v1/discovery/health');
  }

  // ─── Markets ────────────────────────────────────────────

  /**
   * List markets with optional filtering.
   *
   * @param options - Filter and pagination options
   * @returns Markets list
   */
  async listMarkets(options?: {
    provider?: string;
    status?: types.MarketStatus;
    category?: string;
    market_type?: types.MarketType;
    sort_by?: string;
    limit?: number;
    cursor?: string;
  }): Promise<types.MarketsResponse> {
    return this.request('GET', '/upp/v1/markets', {
      query: {
        provider: options?.provider,
        status: options?.status,
        category: options?.category,
        market_type: options?.market_type,
        sort_by: options?.sort_by,
        limit: options?.limit,
        cursor: options?.cursor,
      },
    });
  }

  /**
   * Search markets by query string.
   *
   * @param query - Search query
   * @param options - Additional search options
   * @returns Markets list
   */
  async searchMarkets(
    query: string,
    options?: {
      provider?: string;
      limit?: number;
      cursor?: string;
    }
  ): Promise<types.MarketsResponse> {
    return this.request('GET', '/upp/v1/markets/search', {
      query: {
        q: query,
        provider: options?.provider,
        limit: options?.limit,
        cursor: options?.cursor,
      },
    });
  }

  /**
   * Get a specific market by ID.
   *
   * @param marketId - Market ID (e.g., "upp:kalshi:MELON-240301")
   * @returns Market data
   */
  async getMarket(marketId: string): Promise<types.Market> {
    return this.request('GET', `/upp/v1/markets/${encodeURIComponent(marketId)}`);
  }

  /**
   * Get order book for a market.
   *
   * @param marketId - Market ID
   * @param options - Orderbook options
   * @returns Orderbook snapshot
   */
  async getOrderbook(
    marketId: string,
    options?: {
      outcome?: string;
      depth?: number;
    }
  ): Promise<types.OrderbookResponse> {
    return this.request('GET', `/upp/v1/markets/${encodeURIComponent(marketId)}/orderbook`, {
      query: {
        outcome: options?.outcome,
        depth: options?.depth,
      },
    });
  }

  /**
   * Get merged order book across providers.
   *
   * @param marketId - Market ID
   * @param options - Orderbook options
   * @returns Merged orderbook
   */
  async getMergedOrderbook(
    marketId: string,
    options?: {
      outcome?: string;
      depth?: number;
    }
  ): Promise<types.MergedOrderbookResponse> {
    return this.request('GET', `/upp/v1/markets/${encodeURIComponent(marketId)}/orderbook/merged`, {
      query: {
        outcome: options?.outcome,
        depth: options?.depth,
      },
    });
  }

  /**
   * List available market categories.
   *
   * @returns Categories list
   */
  async listCategories(): Promise<{ categories: string[] }> {
    return this.request('GET', '/upp/v1/markets/categories');
  }

  /**
   * Get resolution information for a market.
   *
   * @param marketId - Market ID
   * @returns Resolution data
   */
  async getResolution(marketId: string): Promise<unknown> {
    return this.request('GET', `/upp/v1/resolutions/${encodeURIComponent(marketId)}`);
  }

  /**
   * List all market resolutions.
   *
   * @returns Resolutions list
   */
  async listResolutions(): Promise<{ resolutions: unknown[] }> {
    return this.request('GET', '/upp/v1/resolutions');
  }

  /**
   * List settlement instruments.
   *
   * @returns Settlement instruments
   */
  async listSettlementInstruments(): Promise<{ instruments: unknown[] }> {
    return this.request('GET', '/upp/v1/settlement/instruments');
  }

  /**
   * List settlement handlers.
   *
   * @returns Settlement handlers
   */
  async listSettlementHandlers(): Promise<{ handlers: unknown[] }> {
    return this.request('GET', '/upp/v1/settlement/handlers');
  }

  // ─── Trading (Protected) ─────────────────────────────────

  /**
   * Create a new order.
   *
   * @param request - Order creation request
   * @returns Created order
   */
  async createOrder(request: types.CreateOrderRequest): Promise<types.Order> {
    return this.request('POST', '/upp/v1/orders', {
      body: request,
      auth: true,
    });
  }

  /**
   * List all orders for the authenticated user.
   *
   * @param options - Filter options
   * @returns Orders list
   */
  async listOrders(options?: {
    provider?: string;
    market_id?: string;
    status?: types.OrderStatus;
    limit?: number;
    cursor?: string;
  }): Promise<types.OrdersResponse> {
    return this.request('GET', '/upp/v1/orders', {
      query: {
        provider: options?.provider,
        market_id: options?.market_id,
        status: options?.status,
        limit: options?.limit,
        cursor: options?.cursor,
      },
      auth: true,
    });
  }

  /**
   * Get a specific order by ID.
   *
   * @param orderId - Order ID
   * @param provider - Optional provider ID
   * @returns Order data
   */
  async getOrder(orderId: string, provider?: string): Promise<types.Order> {
    return this.request('GET', `/upp/v1/orders/${encodeURIComponent(orderId)}`, {
      query: { provider },
      auth: true,
    });
  }

  /**
   * Cancel a specific order.
   *
   * @param orderId - Order ID
   * @param provider - Optional provider ID
   * @returns Cancelled order
   */
  async cancelOrder(orderId: string, provider?: string): Promise<types.Order> {
    return this.request('DELETE', `/upp/v1/orders/${encodeURIComponent(orderId)}`, {
      query: { provider },
      auth: true,
    });
  }

  /**
   * Cancel all orders for a provider.
   *
   * @param provider - Provider ID
   * @param marketId - Optional market ID to limit cancellation
   * @returns Cancelled orders
   */
  async cancelAllOrders(
    provider: string,
    marketId?: string
  ): Promise<types.CancelAllOrdersResponse> {
    return this.request('POST', '/upp/v1/orders/cancel-all', {
      body: { provider, market_id: marketId },
      auth: true,
    });
  }

  /**
   * Estimate order cost and fees.
   *
   * @param request - Order estimation request
   * @returns Order estimate
   */
  async estimateOrder(request: types.EstimateOrderRequest): Promise<types.OrderEstimate> {
    return this.request('POST', '/upp/v1/orders/estimate', {
      body: request,
      auth: true,
    });
  }

  /**
   * List all trades for the authenticated user.
   *
   * @param options - Filter options
   * @returns Trades list
   */
  async listTrades(options?: {
    provider?: string;
    market_id?: string;
    limit?: number;
    cursor?: string;
  }): Promise<types.TradesResponse> {
    return this.request('GET', '/upp/v1/trades', {
      query: {
        provider: options?.provider,
        market_id: options?.market_id,
        limit: options?.limit,
        cursor: options?.cursor,
      },
      auth: true,
    });
  }

  // ─── Portfolio (Protected) ───────────────────────────────

  /**
   * Get all positions for the authenticated user.
   *
   * @param provider - Optional provider filter
   * @returns Positions list
   */
  async listPositions(provider?: string): Promise<types.PositionsResponse> {
    return this.request('GET', '/upp/v1/portfolio/positions', {
      query: { provider },
      auth: true,
    });
  }

  /**
   * Get portfolio summary (total value, PnL, etc.).
   *
   * @param provider - Optional provider filter
   * @returns Portfolio summary
   */
  async getPortfolioSummary(provider?: string): Promise<types.PortfolioSummary> {
    return this.request('GET', '/upp/v1/portfolio/summary', {
      query: { provider },
      auth: true,
    });
  }

  /**
   * Get portfolio balances across currencies.
   *
   * @param provider - Optional provider filter
   * @returns Portfolio balances
   */
  async listPortfolioBalances(provider?: string): Promise<{
    balances: types.PortfolioBalance[];
    total: number;
  }> {
    return this.request('GET', '/upp/v1/portfolio/balances', {
      query: { provider },
      auth: true,
    });
  }

  // ─── MCP (Model Context Protocol) ────────────────────────

  /**
   * List all available MCP tools.
   *
   * @returns MCP tools
   */
  async listMcpTools(): Promise<types.McpToolsResponse> {
    return this.request('GET', '/upp/v1/mcp/tools');
  }

  /**
   * Get MCP schema for all tools.
   *
   * @returns MCP schema
   */
  async getMcpSchema(): Promise<types.McpSchemaResponse> {
    return this.request('GET', '/upp/v1/mcp/schema');
  }

  /**
   * Execute an MCP tool.
   *
   * @param tool - Tool name
   * @param params - Tool parameters
   * @returns Tool result
   */
  async executeMcpTool(
    tool: string,
    params: Record<string, unknown>
  ): Promise<types.McpExecuteResponse> {
    return this.request('POST', '/upp/v1/mcp/execute', {
      body: { tool, params },
    });
  }

  /**
   * Get the agent card (A2A integration).
   *
   * @returns Agent card
   */
  async getAgentCard(): Promise<types.AgentCard> {
    return this.request('GET', '/.well-known/agent.json');
  }
}

/**
 * Error thrown by the UPP API client.
 */
export class UppApiError extends Error {
  constructor(
    message: string,
    public code: string,
    public status: number,
    public details?: Record<string, string>
  ) {
    super(message);
    this.name = 'UppApiError';
  }
}
