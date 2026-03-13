/**
 * UPP SDK WebSocket Client
 *
 * Real-time subscriptions to market prices and orderbook updates.
 * Includes automatic reconnection with exponential backoff.
 */

import * as types from './types';

/**
 * Configuration for the WebSocket client.
 */
export interface WebSocketClientConfig {
  /** WebSocket URL (e.g., "ws://localhost:8080/upp/v1/ws") */
  url: string;
  /** Reconnection options */
  reconnect?: {
    enabled: boolean;
    maxAttempts: number;
    initialDelayMs: number;
    maxDelayMs: number;
    backoffMultiplier: number;
  };
  /** Heartbeat interval in milliseconds (default: 30000) */
  heartbeatInterval?: number;
  /** Custom WebSocket implementation */
  WebSocket?: typeof globalThis.WebSocket;
}

/**
 * Callbacks for WebSocket events.
 */
export interface WebSocketCallbacks {
  /** Called when connected */
  onConnect?: () => void;
  /** Called when disconnected */
  onDisconnect?: () => void;
  /** Called on price updates */
  onPrice?: (update: types.PriceUpdate) => void;
  /** Called on orderbook updates */
  onOrderbook?: (update: types.OrderbookUpdate) => void;
  /** Called on error */
  onError?: (error: Error) => void;
}

/**
 * WebSocket client for real-time market data.
 *
 * @example
 * ```typescript
 * const ws = new UppWebSocket({
 *   url: 'ws://localhost:8080/upp/v1/ws'
 * });
 *
 * ws.on({
 *   onPrice: (update) => console.log('Price:', update),
 *   onOrderbook: (update) => console.log('Orderbook:', update),
 * });
 *
 * await ws.connect();
 * await ws.subscribePrices(['upp:kalshi:MELON-240301']);
 * ```
 */
export class UppWebSocket {
  private url: string;
  private socket?: WebSocket;
  private reconnectConfig: Required<NonNullable<WebSocketClientConfig['reconnect']>>;
  private heartbeatInterval: number;
  private WebSocketImpl: typeof globalThis.WebSocket;
  private callbacks: WebSocketCallbacks = {};
  private reconnectAttempts = 0;
  private heartbeatHandle?: NodeJS.Timeout;
  private reconnectHandle?: NodeJS.Timeout;
  private connected = false;
  private subscriptions = new Map<string, Set<string>>();
  private requestId = 0;

  constructor(config: WebSocketClientConfig) {
    this.url = config.url;
    this.heartbeatInterval = config.heartbeatInterval ?? 30000;
    this.WebSocketImpl = config.WebSocket ?? globalThis.WebSocket;

    this.reconnectConfig = {
      enabled: config.reconnect?.enabled ?? true,
      maxAttempts: config.reconnect?.maxAttempts ?? 10,
      initialDelayMs: config.reconnect?.initialDelayMs ?? 1000,
      maxDelayMs: config.reconnect?.maxDelayMs ?? 30000,
      backoffMultiplier: config.reconnect?.backoffMultiplier ?? 2,
    };
  }

  /**
   * Register event callbacks.
   *
   * @param callbacks - Callback handlers
   */
  on(callbacks: WebSocketCallbacks): void {
    this.callbacks = { ...this.callbacks, ...callbacks };
  }

  /**
   * Connect to the WebSocket server.
   *
   * @returns Promise that resolves when connected
   */
  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        this.socket = new this.WebSocketImpl(this.url);

        this.socket.onopen = () => {
          this.connected = true;
          this.reconnectAttempts = 0;
          this.startHeartbeat();
          this.callbacks.onConnect?.();
          resolve();
        };

        this.socket.onmessage = (event) => {
          this.handleMessage(event.data);
        };

        this.socket.onerror = (error) => {
          const err = new Error(`WebSocket error: ${error}`);
          this.callbacks.onError?.(err);
          reject(err);
        };

        this.socket.onclose = () => {
          this.connected = false;
          this.stopHeartbeat();
          this.callbacks.onDisconnect?.();
          this.attemptReconnect();
        };
      } catch (error) {
        reject(error);
      }
    });
  }

  /**
   * Disconnect from the WebSocket server.
   */
  disconnect(): void {
    this.stopHeartbeat();
    this.clearReconnectHandle();
    if (this.socket) {
      this.socket.close();
      this.socket = undefined;
    }
    this.connected = false;
  }

  /**
   * Subscribe to price updates for specified markets.
   *
   * @param marketIds - Market IDs to subscribe to
   * @param intervalMs - Update interval in milliseconds (default: 1000)
   */
  async subscribePrices(marketIds: string[], intervalMs: number = 1000): Promise<void> {
    const id = ++this.requestId;
    const sub: types.PriceSubscription = {
      channel: 'prices',
      market_ids: marketIds,
      interval_ms: intervalMs,
    };

    const request: types.JsonRpcRequest = {
      jsonrpc: '2.0',
      id,
      method: 'subscribe_prices',
      params: sub,
    };

    return this.sendRequest(request, id);
  }

  /**
   * Subscribe to orderbook updates for specified markets.
   *
   * @param marketIds - Market IDs to subscribe to
   * @param depth - Orderbook depth (default: 10)
   * @param intervalMs - Update interval in milliseconds (default: 2000)
   */
  async subscribeOrderbook(
    marketIds: string[],
    depth: number = 10,
    intervalMs: number = 2000
  ): Promise<void> {
    const id = ++this.requestId;
    const sub: types.OrderbookSubscription = {
      channel: 'orderbook',
      market_ids: marketIds,
      depth,
      interval_ms: intervalMs,
    };

    const request: types.JsonRpcRequest = {
      jsonrpc: '2.0',
      id,
      method: 'subscribe_orderbook',
      params: sub,
    };

    return this.sendRequest(request, id);
  }

  /**
   * Unsubscribe from a channel.
   *
   * @param channel - Channel name ("prices" or "orderbook")
   * @param marketIds - Market IDs to unsubscribe from
   */
  async unsubscribe(channel: string, marketIds: string[]): Promise<void> {
    const id = ++this.requestId;
    const request: types.JsonRpcRequest = {
      jsonrpc: '2.0',
      id,
      method: 'unsubscribe',
      params: {
        channel,
        market_ids: marketIds,
      },
    };

    return this.sendRequest(request, id);
  }

  /**
   * Get a specific market (one-off request).
   *
   * @param marketId - Market ID
   * @returns Market data
   */
  async getMarket(marketId: string): Promise<types.Market> {
    const id = ++this.requestId;
    const request: types.JsonRpcRequest = {
      jsonrpc: '2.0',
      id,
      method: 'get_market',
      params: { market_id: marketId },
    };

    return this.sendRpcRequest(request, id);
  }

  /**
   * Check if WebSocket is connected.
   *
   * @returns Connection state
   */
  isConnected(): boolean {
    return this.connected && this.socket?.readyState === 1;
  }

  /**
   * Get current subscriptions.
   *
   * @returns Subscriptions map
   */
  getSubscriptions(): Map<string, Set<string>> {
    return new Map(this.subscriptions);
  }

  // ─── Private Methods ────────────────────────────────────

  /**
   * Send a JSON-RPC request and wait for response.
   *
   * @internal
   */
  private sendRequest(request: types.JsonRpcRequest, id: number | string): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.socket || this.socket.readyState !== 1) {
        reject(new Error('WebSocket not connected'));
        return;
      }

      const timeoutHandle = setTimeout(() => {
        reject(new Error(`Request ${id} timed out`));
      }, 10000);

      const originalOnMessage = this.socket.onmessage;
      const handler = (event: MessageEvent) => {
        try {
          const response = JSON.parse(event.data) as types.JsonRpcResponse;
          if (response.id === id) {
            clearTimeout(timeoutHandle);
            if (response.error) {
              reject(new Error(response.error.message));
            } else {
              resolve();
            }
            // Restore original handler
            if (originalOnMessage) {
              this.socket!.onmessage = originalOnMessage;
            }
          }
        } catch (error) {
          // Not a JSON-RPC response, might be a fan-out message
        }
      };

      this.socket.onmessage = handler;
      this.socket.send(JSON.stringify(request));
    });
  }

  /**
   * Send a JSON-RPC request that expects a result.
   *
   * @internal
   */
  private sendRpcRequest<T>(request: types.JsonRpcRequest, id: number | string): Promise<T> {
    return new Promise((resolve, reject) => {
      if (!this.socket || this.socket.readyState !== 1) {
        reject(new Error('WebSocket not connected'));
        return;
      }

      const timeoutHandle = setTimeout(() => {
        reject(new Error(`Request ${id} timed out`));
      }, 10000);

      const originalOnMessage = this.socket.onmessage;
      const handler = (event: MessageEvent) => {
        try {
          const response = JSON.parse(event.data) as types.JsonRpcResponse;
          if (response.id === id) {
            clearTimeout(timeoutHandle);
            if (response.error) {
              reject(new Error(response.error.message));
            } else {
              resolve(response.result as T);
            }
            // Restore original handler
            if (originalOnMessage) {
              this.socket!.onmessage = originalOnMessage;
            }
          }
        } catch (error) {
          // Not a JSON-RPC response, might be a fan-out message
        }
      };

      this.socket.onmessage = handler;
      this.socket.send(JSON.stringify(request));
    });
  }

  /**
   * Handle incoming WebSocket messages.
   *
   * @internal
   */
  private handleMessage(data: string): void {
    try {
      const message = JSON.parse(data);

      // Check if it's a fan-out message or JSON-RPC response
      if ('channel' in message) {
        const fanOut = message as types.FanOutMessage;
        if (fanOut.channel === 'prices') {
          this.callbacks.onPrice?.(fanOut as types.PriceUpdate);
        } else if (fanOut.channel === 'orderbook') {
          this.callbacks.onOrderbook?.(fanOut as types.OrderbookUpdate);
        }
      }
    } catch (error) {
      this.callbacks.onError?.(new Error(`Failed to parse message: ${error}`));
    }
  }

  /**
   * Attempt to reconnect with exponential backoff.
   *
   * @internal
   */
  private attemptReconnect(): void {
    if (!this.reconnectConfig.enabled) return;
    if (this.reconnectAttempts >= this.reconnectConfig.maxAttempts) {
      this.callbacks.onError?.(
        new Error(`Failed to reconnect after ${this.reconnectAttempts} attempts`)
      );
      return;
    }

    const delayMs = Math.min(
      this.reconnectConfig.initialDelayMs *
        Math.pow(this.reconnectConfig.backoffMultiplier, this.reconnectAttempts),
      this.reconnectConfig.maxDelayMs
    );

    this.reconnectHandle = setTimeout(() => {
      this.reconnectAttempts++;
      this.connect().catch((error) => {
        this.callbacks.onError?.(error);
      });
    }, delayMs);
  }

  /**
   * Start heartbeat timer.
   *
   * @internal
   */
  private startHeartbeat(): void {
    this.heartbeatHandle = setInterval(() => {
      if (this.isConnected()) {
        const id = ++this.requestId;
        const request: types.JsonRpcRequest = {
          jsonrpc: '2.0',
          id,
          method: 'ping',
          params: {},
        };
        this.socket?.send(JSON.stringify(request));
      }
    }, this.heartbeatInterval);
  }

  /**
   * Stop heartbeat timer.
   *
   * @internal
   */
  private stopHeartbeat(): void {
    if (this.heartbeatHandle) {
      clearInterval(this.heartbeatHandle);
      this.heartbeatHandle = undefined;
    }
  }

  /**
   * Clear reconnect handle.
   *
   * @internal
   */
  private clearReconnectHandle(): void {
    if (this.reconnectHandle) {
      clearTimeout(this.reconnectHandle);
      this.reconnectHandle = undefined;
    }
  }
}
