"""
UPP Python SDK Client

A type-safe client for the Universal Prediction Protocol API.
Provides unified access to prediction market aggregation, trading,
arbitrage detection, and real-time data streaming.
"""

from dataclasses import dataclass
from typing import Optional, List, Dict, Any
from enum import Enum
import json

try:
    import httpx
    HAS_HTTPX = True
except ImportError:
    HAS_HTTPX = False

try:
    import requests
    HAS_REQUESTS = True
except ImportError:
    HAS_REQUESTS = False

if not HAS_HTTPX and not HAS_REQUESTS:
    raise ImportError("Either 'httpx' or 'requests' is required. Install with: pip install httpx")


class UppError(Exception):
    """UPP API error"""
    def __init__(self, code: str, message: str, request_id: Optional[str] = None):
        self.code = code
        self.message = message
        self.request_id = request_id
        super().__init__(f"{code}: {message}" + (f" (request_id: {request_id})" if request_id else ""))


class OrderSide(str, Enum):
    """Order side"""
    BUY = "buy"
    SELL = "sell"


class OrderType(str, Enum):
    """Order type"""
    LIMIT = "limit"
    MARKET = "market"


class MarketStatus(str, Enum):
    """Market status"""
    OPEN = "open"
    CLOSED = "closed"
    SETTLED = "settled"


@dataclass
class Outcome:
    """Market outcome"""
    id: str
    name: str
    price: float


@dataclass
class Market:
    """Market details"""
    id: str
    name: str
    provider: str
    status: str
    price: float
    volume: float
    outcomes: List[Outcome]
    created_at: str


@dataclass
class OrderbookLevel:
    """Orderbook bid/ask level"""
    price: float
    quantity: float


@dataclass
class Orderbook:
    """Full orderbook"""
    market_id: str
    bids: List[OrderbookLevel]
    asks: List[OrderbookLevel]
    timestamp: Optional[str] = None


@dataclass
class Candle:
    """OHLCV candlestick data"""
    timestamp: int
    open: float
    high: float
    low: float
    close: float
    volume: float


@dataclass
class CreateOrderRequest:
    """Order creation request"""
    market_id: str
    side: OrderSide
    price: float
    quantity: float
    order_type: OrderType = OrderType.LIMIT


@dataclass
class Order:
    """Order details"""
    id: str
    market_id: str
    side: str
    price: float
    quantity: float
    filled: float
    status: str
    created_at: str
    updated_at: Optional[str] = None


@dataclass
class Position:
    """Open position"""
    market_id: str
    outcome_id: str
    quantity: float
    entry_price: float
    current_price: float
    pnl: float
    pnl_percent: float


@dataclass
class PortfolioSummary:
    """Portfolio summary with P&L"""
    total_balance: float
    available_balance: float
    positions: List[Position]
    total_pnl: float
    win_rate: float
    sharpe_ratio: float


@dataclass
class ArbitrageOpportunity:
    """Cross-provider arbitrage opportunity"""
    id: str
    market_id: str
    spread_percent: float
    buy_provider: str
    buy_price: float
    sell_provider: str
    sell_price: float
    estimated_profit: float
    timestamp: str


@dataclass
class HealthResponse:
    """Health check response"""
    status: str
    version: str
    uptime: str


@dataclass
class Provider:
    """Provider information"""
    id: str
    name: str
    status: str
    markets_count: Optional[int] = None


@dataclass
class ListResponse:
    """List response with pagination"""
    data: List[Any]
    cursor: Optional[str] = None
    limit: Optional[int] = None
    total: Optional[int] = None


class UppClient:
    """
    UPP API Client

    Usage:
        client = UppClient(base_url="https://api.upp.dev", api_key="your-api-key")
        health = await client.health()
        markets = await client.list_markets()
    """

    def __init__(self, base_url: str, api_key: Optional[str] = None):
        """Initialize UPP client"""
        self.base_url = base_url.rstrip('/')
        self.api_key = api_key
        self._session = None
        self._use_async = HAS_HTTPX
        
        self.default_headers = {
            'Content-Type': 'application/json',
        }
        if self.api_key:
            self.default_headers['X-API-Key'] = self.api_key

    def _get_session(self):
        """Get or create HTTP session"""
        if self._session is None:
            if self._use_async and HAS_HTTPX:
                self._session = httpx.Client(headers=self.default_headers)
            elif HAS_REQUESTS:
                self._session = requests.Session()
                self._session.headers.update(self.default_headers)
        return self._session

    def _parse_error(self, data: Dict[str, Any]) -> UppError:
        """Parse error response"""
        error = data.get('error', {})
        return UppError(
            code=error.get('code', 'HTTP_ERROR'),
            message=error.get('message', 'Unknown error'),
            request_id=error.get('request_id')
        )

    def _request(self, method: str, path: str, **kwargs) -> Dict[str, Any]:
        """Make HTTP request"""
        url = f"{self.base_url}{path}"
        session = self._get_session()
        
        if self._use_async and isinstance(session, httpx.Client):
            response = session.request(method, url, **kwargs)
        else:
            response = session.request(method, url, **kwargs)
        
        try:
            data = response.json()
        except:
            data = {}

        if response.status_code >= 400:
            raise self._parse_error(data)

        return data

    def health(self) -> HealthResponse:
        """Health check — all providers"""
        data = self._request('GET', '/health')
        return HealthResponse(**data)

    def health_provider(self, provider: str) -> Dict[str, Any]:
        """Health check — single provider"""
        return self._request('GET', f'/upp/v1/discovery/health/{provider}')

    def list_providers(self) -> ListResponse:
        """List all registered prediction providers"""
        data = self._request('GET', '/upp/v1/discovery/providers')
        return ListResponse(data=data.get('data', []))

    def get_provider_manifest(self, provider: str) -> Dict[str, Any]:
        """Get provider manifest"""
        return self._request('GET', f'/upp/v1/discovery/manifest/{provider}')

    def list_markets(self, provider: Optional[str] = None, status: Optional[str] = None,
                     category: Optional[str] = None, limit: Optional[int] = None,
                     cursor: Optional[str] = None) -> ListResponse:
        """List all markets across providers"""
        params = {}
        if provider:
            params['provider'] = provider
        if status:
            params['status'] = status
        if category:
            params['category'] = category
        if limit:
            params['limit'] = limit
        if cursor:
            params['cursor'] = cursor
        
        data = self._request('GET', '/upp/v1/markets', params=params)
        return ListResponse(data=data.get('data', []))

    def search_markets(self, query: str, provider: Optional[str] = None,
                      limit: int = 20) -> ListResponse:
        """Search markets by keyword"""
        params = {'q': query, 'limit': limit}
        if provider:
            params['provider'] = provider
        
        data = self._request('GET', '/upp/v1/markets/search', params=params)
        return ListResponse(data=data.get('data', []))

    def get_market(self, market_id: str) -> Market:
        """Get market details by ID"""
        data = self._request('GET', f'/upp/v1/markets/{market_id}')
        return Market(**data)

    def get_orderbook(self, market_id: str) -> Orderbook:
        """Get orderbook for a market"""
        data = self._request('GET', f'/upp/v1/markets/{market_id}/orderbook')
        return Orderbook(**data)

    def get_merged_orderbook(self, market_id: str) -> Orderbook:
        """Get merged orderbook across providers"""
        data = self._request('GET', f'/upp/v1/markets/{market_id}/orderbook/merged')
        return Orderbook(**data)

    def list_categories(self) -> ListResponse:
        """List market categories"""
        data = self._request('GET', '/upp/v1/markets/categories')
        return ListResponse(data=data.get('data', []))

    def list_arbitrage(self) -> ListResponse:
        """List current arbitrage opportunities"""
        data = self._request('GET', '/upp/v1/arbitrage')
        return ListResponse(data=data.get('data', []))

    def arbitrage_summary(self) -> Dict[str, Any]:
        """Get arbitrage summary statistics"""
        return self._request('GET', '/upp/v1/arbitrage/summary')

    def arbitrage_history(self, limit: int = 100) -> Dict[str, Any]:
        """Get historical arbitrage data"""
        return self._request('GET', '/upp/v1/arbitrage/history', params={'limit': limit})

    def get_candles(self, market_id: str, outcome: Optional[str] = None,
                   resolution: str = '1h', limit: Optional[int] = None,
                   start: Optional[int] = None, end: Optional[int] = None) -> List[Candle]:
        """Get candlestick data for a market"""
        params = {'resolution': resolution}
        if outcome:
            params['outcome'] = outcome
        if limit:
            params['limit'] = limit
        if start:
            params['start'] = start
        if end:
            params['end'] = end
        
        data = self._request('GET', f'/upp/v1/markets/{market_id}/candles', params=params)
        return [Candle(**c) for c in (data if isinstance(data, list) else data.get('data', []))]

    def get_latest_candle(self, market_id: str, outcome: Optional[str] = None,
                         resolution: str = '1h') -> Candle:
        """Get latest candle for a market"""
        params = {'resolution': resolution}
        if outcome:
            params['outcome'] = outcome
        
        data = self._request('GET', f'/upp/v1/markets/{market_id}/candles/latest', params=params)
        return Candle(**data)

    def price_index_stats(self) -> Dict[str, Any]:
        """Get price index statistics"""
        return self._request('GET', '/upp/v1/price-index/stats')

    def list_resolutions(self) -> Dict[str, Any]:
        """List available resolutions"""
        return self._request('GET', '/upp/v1/resolutions')

    def get_resolution(self, market_id: str) -> Dict[str, Any]:
        """Get resolutions for a market"""
        return self._request('GET', f'/upp/v1/resolutions/{market_id}')

    def create_order(self, order: CreateOrderRequest) -> Order:
        """Create a new order (requires API key)"""
        data = self._request('POST', '/upp/v1/orders',
                           json={
                               'market_id': order.market_id,
                               'side': order.side.value,
                               'price': order.price,
                               'quantity': order.quantity,
                               'order_type': order.order_type.value
                           })
        return Order(**data)

    def list_orders(self, provider: Optional[str] = None, status: Optional[str] = None,
                   limit: Optional[int] = None) -> ListResponse:
        """List orders (requires API key)"""
        params = {}
        if provider:
            params['provider'] = provider
        if status:
            params['status'] = status
        if limit:
            params['limit'] = limit
        
        data = self._request('GET', '/upp/v1/orders', params=params)
        return ListResponse(data=data.get('data', []))

    def get_order(self, order_id: str) -> Order:
        """Get order details by ID (requires API key)"""
        data = self._request('GET', f'/upp/v1/orders/{order_id}')
        return Order(**data)

    def cancel_order(self, order_id: str) -> Dict[str, Any]:
        """Cancel an order (requires API key)"""
        return self._request('DELETE', f'/upp/v1/orders/{order_id}')

    def cancel_all_orders(self) -> Dict[str, Any]:
        """Cancel all open orders (requires API key)"""
        return self._request('POST', '/upp/v1/orders/cancel-all')

    def estimate_order(self, order: CreateOrderRequest) -> Dict[str, Any]:
        """Estimate order cost without placing (requires API key)"""
        return self._request('POST', '/upp/v1/orders/estimate',
                           json={
                               'market_id': order.market_id,
                               'side': order.side.value,
                               'price': order.price,
                               'quantity': order.quantity,
                               'order_type': order.order_type.value
                           })

    def list_trades(self, limit: int = 50) -> ListResponse:
        """List trade executions (requires API key)"""
        data = self._request('GET', '/upp/v1/trades', params={'limit': limit})
        return ListResponse(data=data.get('data', []))

    def list_positions(self) -> ListResponse:
        """List open positions (requires API key)"""
        data = self._request('GET', '/upp/v1/portfolio/positions')
        return ListResponse(data=data.get('data', []))

    def get_portfolio_summary(self) -> PortfolioSummary:
        """Get portfolio summary with P&L (requires API key)"""
        data = self._request('GET', '/upp/v1/portfolio/summary')
        return PortfolioSummary(**data)

    def list_balances(self) -> Dict[str, Any]:
        """Get account balances (requires API key)"""
        return self._request('GET', '/upp/v1/portfolio/balances')

    def get_portfolio_analytics(self) -> Dict[str, Any]:
        """Get portfolio analytics (requires API key)"""
        return self._request('GET', '/upp/v1/portfolio/analytics')

    def feed_status(self) -> Dict[str, Any]:
        """Get live feed connection status"""
        return self._request('GET', '/upp/v1/feeds/status')

    def feed_stats(self) -> Dict[str, Any]:
        """Get live feed statistics"""
        return self._request('GET', '/upp/v1/feeds/stats')

    def subscribe_feed(self, provider_id: str, market_ids: List[str]) -> Dict[str, Any]:
        """Subscribe to market feeds (requires API key)"""
        return self._request('POST', '/upp/v1/feeds/subscribe',
                           json={
                               'provider_id': provider_id,
                               'market_ids': market_ids
                           })

    def list_strategies(self) -> Dict[str, Any]:
        """List available backtest strategies"""
        return self._request('GET', '/upp/v1/backtest/strategies')

    def run_backtest(self, strategy: str, market_id: str,
                    params: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """Run a backtest"""
        data = {'strategy': strategy, 'market_id': market_id}
        if params:
            data['params'] = params
        return self._request('POST', '/upp/v1/backtest/run', json=data)

    def compare_strategies(self, params: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """Compare multiple strategies"""
        return self._request('POST', '/upp/v1/backtest/compare', json=params or {})

    def ingestion_stats(self) -> Dict[str, Any]:
        """Get ingestion pipeline statistics"""
        return self._request('GET', '/upp/v1/ingestion/stats')

    def list_instruments(self) -> Dict[str, Any]:
        """List settlement instruments"""
        return self._request('GET', '/upp/v1/settlement/instruments')

    def list_handlers(self) -> Dict[str, Any]:
        """List settlement handlers"""
        return self._request('GET', '/upp/v1/settlement/handlers')

    def list_mcp_tools(self) -> Dict[str, Any]:
        """List available MCP tools"""
        return self._request('GET', '/upp/v1/mcp/tools')

    def execute_mcp_tool(self, tool: str, params: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """Execute an MCP tool"""
        return self._request('POST', '/upp/v1/mcp/execute',
                           json={'tool': tool, 'params': params or {}})

    def mcp_schema(self) -> Dict[str, Any]:
        """Get MCP OpenAPI schema"""
        return self._request('GET', '/upp/v1/mcp/schema')

    def ready(self) -> Dict[str, Any]:
        """Readiness check"""
        return self._request('GET', '/ready')

    def metrics(self) -> str:
        """Get Prometheus metrics"""
        url = f"{self.base_url}/metrics"
        session = self._get_session()
        
        if self._use_async and isinstance(session, httpx.Client):
            response = session.get(url)
        else:
            response = session.get(url)
        
        if response.status_code >= 400:
            raise UppError('HTTP_ERROR', f'HTTP {response.status_code}')
        
        return response.text

    def close(self):
        """Close HTTP session"""
        if self._session:
            self._session.close()
            self._session = None

    def __enter__(self):
        """Context manager entry"""
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit"""
        self.close()


__all__ = [
    'UppClient',
    'UppError',
    'Market',
    'Outcome',
    'Orderbook',
    'OrderbookLevel',
    'Candle',
    'Order',
    'CreateOrderRequest',
    'Position',
    'PortfolioSummary',
    'ArbitrageOpportunity',
    'HealthResponse',
    'Provider',
    'ListResponse',
    'OrderSide',
    'OrderType',
    'MarketStatus',
]
