/**
 * UPP SDK MCP (Model Context Protocol) Helpers
 *
 * Utilities for working with MCP tools and agent integration.
 */

import { UppClient } from './client';
import * as types from './types';

/**
 * MCP helper for working with tools and schema.
 *
 * @example
 * ```typescript
 * const client = new UppClient({ baseUrl: 'http://localhost:8080' });
 * const mcp = new McpHelper(client);
 *
 * const tools = await mcp.listTools();
 * const schema = await mcp.getSchema();
 * const result = await mcp.executeTool('get_market', { market_id: 'upp:kalshi:MELON-240301' });
 * ```
 */
export class McpHelper {
  constructor(private client: UppClient) {}

  /**
   * List all available MCP tools.
   *
   * @returns Array of tool definitions
   */
  async listTools(): Promise<types.McpTool[]> {
    const response = await this.client.listMcpTools();
    return response.tools;
  }

  /**
   * Get the MCP schema (OpenAPI-like) for all tools.
   *
   * @returns Schema definition
   */
  async getSchema(): Promise<types.McpSchemaResponse> {
    return this.client.getMcpSchema();
  }

  /**
   * Execute an MCP tool.
   *
   * @param tool - Tool name
   * @param params - Tool parameters
   * @returns Tool result
   */
  async executeTool(tool: string, params: Record<string, unknown>): Promise<unknown> {
    const response = await this.client.executeMcpTool(tool, params);
    return response.result;
  }

  /**
   * Find a tool by name.
   *
   * @param name - Tool name
   * @returns Tool definition or undefined
   */
  async findTool(name: string): Promise<types.McpTool | undefined> {
    const tools = await this.listTools();
    return tools.find((t) => t.name === name);
  }

  /**
   * Get the schema for a specific tool.
   *
   * @param toolName - Tool name
   * @returns Tool input schema or undefined
   */
  async getToolSchema(toolName: string): Promise<Record<string, unknown> | undefined> {
    const tool = await this.findTool(toolName);
    return tool?.input_schema;
  }

  /**
   * List all available tool names.
   *
   * @returns Array of tool names
   */
  async listToolNames(): Promise<string[]> {
    const tools = await this.listTools();
    return tools.map((t) => t.name);
  }
}

/**
 * Agent card provider for A2A (Agent-to-Agent) integration.
 */
export class AgentCardProvider {
  constructor(private client: UppClient) {}

  /**
   * Get the agent card for A2A integration.
   *
   * @returns Agent card definition
   */
  async getAgentCard(): Promise<types.AgentCard> {
    return this.client.getAgentCard();
  }

  /**
   * Build a complete agent card with MCP tools.
   *
   * @returns Complete agent card with tools
   */
  async buildAgentCard(): Promise<types.AgentCard> {
    const [card, tools] = await Promise.all([
      this.client.getAgentCard(),
      this.client.listMcpTools(),
    ]);

    return {
      ...card,
      tools: tools.tools,
    };
  }
}
