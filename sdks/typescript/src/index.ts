/**
 * UPP SDK for TypeScript
 *
 * Auto-generated client library for the Universal Prediction Protocol gateway.
 * Provides typed access to all REST API endpoints, WebSocket subscriptions, and MCP tools.
 *
 * @packageDocumentation
 */

// ─── Main Client ────────────────────────────────────────────

export { UppClient, UppApiError } from './client';
export type { UppClientConfig } from './client';

// ─── WebSocket ──────────────────────────────────────────────

export { UppWebSocket } from './websocket';
export type { WebSocketClientConfig, WebSocketCallbacks } from './websocket';

// ─── MCP Integration ────────────────────────────────────────

export { McpHelper, AgentCardProvider } from './mcp';

// ─── Types ──────────────────────────────────────────────────

export type * from './types';

// ─── Version ────────────────────────────────────────────────

/**
 * SDK version
 */
export const VERSION = '1.0.0';
