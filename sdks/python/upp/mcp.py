"""
UPP SDK MCP (Model Context Protocol) Helpers

Utilities for working with MCP tools and agent integration.
"""

from typing import Any, Dict, List, Optional

from .client import UppClient
from .types import McpTool


class McpHelper:
    """
    MCP helper for working with tools and schema.

    Example:
        >>> client = UppClient(base_url='http://localhost:8080')
        >>> mcp = McpHelper(client)
        >>> tools = await mcp.list_tools()
        >>> result = await mcp.execute_tool('get_market', {'market_id': 'upp:kalshi:MELON-240301'})
    """

    def __init__(self, client: UppClient):
        """
        Initialize MCP helper.

        Args:
            client: UPP client instance
        """
        self.client = client

    async def list_tools(self) -> List[McpTool]:
        """
        List all available MCP tools.

        Returns:
            Array of tool definitions
        """
        response = await self.client.list_mcp_tools()
        return response.tools

    async def get_schema(self) -> Dict[str, Any]:
        """
        Get the MCP schema (OpenAPI-like) for all tools.

        Returns:
            Schema definition
        """
        response = await self.client.get_mcp_schema()
        return response.model_dump()

    async def execute_tool(
        self, tool: str, params: Dict[str, Any]
    ) -> Any:
        """
        Execute an MCP tool.

        Args:
            tool: Tool name
            params: Tool parameters

        Returns:
            Tool result
        """
        response = await self.client.execute_mcp_tool(tool, params)
        return response.result

    async def find_tool(self, name: str) -> Optional[McpTool]:
        """
        Find a tool by name.

        Args:
            name: Tool name

        Returns:
            Tool definition or None
        """
        tools = await self.list_tools()
        for tool in tools:
            if tool.name == name:
                return tool
        return None

    async def get_tool_schema(
        self, tool_name: str
    ) -> Optional[Dict[str, Any]]:
        """
        Get the schema for a specific tool.

        Args:
            tool_name: Tool name

        Returns:
            Tool input schema or None
        """
        tool = await self.find_tool(tool_name)
        if tool:
            return tool.input_schema
        return None

    async def list_tool_names(self) -> List[str]:
        """
        List all available tool names.

        Returns:
            Array of tool names
        """
        tools = await self.list_tools()
        return [tool.name for tool in tools]


class AgentCardProvider:
    """
    Agent card provider for A2A (Agent-to-Agent) integration.
    """

    def __init__(self, client: UppClient):
        """
        Initialize agent card provider.

        Args:
            client: UPP client instance
        """
        self.client = client

    async def get_agent_card(self) -> Dict[str, Any]:
        """
        Get the agent card for A2A integration.

        Returns:
            Agent card definition
        """
        return await self.client.get_agent_card()

    async def build_agent_card(self) -> Dict[str, Any]:
        """
        Build a complete agent card with MCP tools.

        Returns:
            Complete agent card with tools
        """
        card = await self.client.get_agent_card()
        tools_response = await self.client.list_mcp_tools()

        return {
            **card,
            "tools": [tool.model_dump() for tool in tools_response.tools],
        }
