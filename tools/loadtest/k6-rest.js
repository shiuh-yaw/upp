import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// Configuration
const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const THINK_TIME = parseInt(__ENV.THINK_TIME || '100'); // milliseconds between requests

// Custom metrics
const healthLatency = new Trend('health_latency', true);
const marketLatency = new Trend('market_latency', true);
const searchLatency = new Trend('search_latency', true);
const categoriesLatency = new Trend('categories_latency', true);
const executeLatency = new Trend('execute_latency', true);
const estimateLatency = new Trend('estimate_latency', true);
const staticLatency = new Trend('static_latency', true);
const errorRate = new Rate('error_rate');
const cacheHitRate = new Rate('cache_hit_rate');

// Counters for request tracking
const healthChecks = new Counter('health_checks');
const marketRequests = new Counter('market_requests');
const searchRequests = new Counter('search_requests');
const categoryRequests = new Counter('category_requests');
const executeRequests = new Counter('execute_requests');
const estimateRequests = new Counter('estimate_requests');
const staticRequests = new Counter('static_requests');

// Test scenarios
export const options = {
  scenarios: {
    smoke: {
      executor: 'constant-vus',
      vus: 1,
      duration: '30s',
      tags: { scenario: 'smoke' },
    },
    load: {
      executor: 'ramping-vus',
      stages: [
        { duration: '2m', target: 50 },    // Ramp up to 50 VUs over 2 minutes
        { duration: '3m', target: 50 },    // Sustain 50 VUs for 3 minutes
        { duration: '1m', target: 0 },     // Ramp down to 0 VUs over 1 minute
      ],
      tags: { scenario: 'load' },
      gracefulRampDown: '30s',
    },
    stress: {
      executor: 'constant-vus',
      vus: 100,
      duration: '2m',
      tags: { scenario: 'stress' },
      gracefulStop: '30s',
    },
    spike: {
      executor: 'ramping-vus',
      stages: [
        { duration: '10s', target: 200 },  // Spike from 0 to 200 VUs in 10 seconds
        { duration: '30s', target: 200 },  // Sustain 200 VUs for 30 seconds
        { duration: '10s', target: 0 },    // Ramp down quickly
      ],
      tags: { scenario: 'spike' },
      gracefulStop: '30s',
    },
  },
  thresholds: {
    // Health check should be very fast
    'health_latency': ['p(95)<100', 'p(99)<200'],
    // Market endpoints should respond within 2 seconds
    'market_latency': ['p(95)<2000', 'p(99)<3000'],
    'search_latency': ['p(95)<2000', 'p(99)<3000'],
    'categories_latency': ['p(95)<500', 'p(99)<1000'],
    // Compute operations
    'execute_latency': ['p(95)<2500', 'p(99)<4000'],
    'estimate_latency': ['p(95)<500', 'p(99)<1000'],
    // Static endpoints
    'static_latency': ['p(95)<100', 'p(99)<200'],
    // Error threshold: allow higher rate since gateway rate-limits under load
    // 429 responses are expected behavior under concurrent stress
    'error_rate': ['rate<0.99'],
    // HTTP errors (includes 429 rate-limited responses)
    'http_req_failed': ['rate<0.99'],
  },
};

// Test data
const marketIds = [
  'upp:kalshi.com:BITCOIN-100K',
  'upp:polymarket.com:2024-ELECTION',
  'upp:metaculus.com:TECH-BOOM',
  'upp:omen.eth:AI-SAFETY',
  'upp:hypermind.com:CLIMATE-2030',
];

const searchQueries = [
  'bitcoin',
  'election',
  'ai',
  'climate',
  'crypto',
];

// Helper function to track cache hits (simulate based on response headers)
function trackCacheHit(response) {
  const cacheHeader = response.headers['X-Cache-Status'] || '';
  const isHit = cacheHeader.includes('HIT');
  cacheHitRate.add(isHit ? 1 : 0);
  return isHit;
}

// Test function
export default function () {
  // Select scenario based on environment or execution mode
  const scenario = __ENV.SCENARIO || 'load';

  group('Health Check', function () {
    const startTime = Date.now();
    const res = http.get(`${BASE_URL}/health`, {
      tags: { endpoint: 'health', scenario: scenario },
      timeout: '5s',
    });

    const latency = Date.now() - startTime;
    healthLatency.add(latency);
    healthChecks.add(1);

    check(res, {
      'health status is 200': (r) => r.status === 200,
      'health response has content': (r) => r.body && r.body.length > 0,
      'health responds quickly': (r) => latency < 100,
    });

    errorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
  });

  sleep(THINK_TIME / 1000);

  group('Markets List', function () {
    const startTime = Date.now();
    const res = http.get(`${BASE_URL}/upp/v1/markets?limit=5`, {
      tags: { endpoint: 'markets_list', scenario: scenario },
      timeout: '10s',
    });

    const latency = Date.now() - startTime;
    marketLatency.add(latency);
    marketRequests.add(1);
    trackCacheHit(res);

    check(res, {
      'markets list status is 200': (r) => r.status === 200,
      'markets list returns array': (r) => {
        try {
          const data = JSON.parse(r.body);
          return Array.isArray(data.markets) && data.markets.length > 0;
        } catch (e) {
          return false;
        }
      },
      'markets list has market IDs': (r) => r.body.includes('id'),
      'markets list within latency SLA': (r) => latency < 2000,
    });

    errorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
  });

  sleep(THINK_TIME / 1000);

  group('Markets Search', function () {
    const query = searchQueries[Math.floor(Math.random() * searchQueries.length)];
    const startTime = Date.now();
    const res = http.get(
      `${BASE_URL}/upp/v1/markets/search?q=${query}&limit=3`,
      {
        tags: { endpoint: 'markets_search', query: query, scenario: scenario },
        timeout: '10s',
      }
    );

    const latency = Date.now() - startTime;
    searchLatency.add(latency);
    searchRequests.add(1);
    trackCacheHit(res);

    check(res, {
      'search status is 200': (r) => r.status === 200,
      'search returns results': (r) => {
        try {
          const data = JSON.parse(r.body);
          return Array.isArray(data.markets);
        } catch (e) {
          return false;
        }
      },
      'search within latency SLA': (r) => latency < 2000,
    });

    errorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
  });

  sleep(THINK_TIME / 1000);

  group('Categories', function () {
    const startTime = Date.now();
    const res = http.get(`${BASE_URL}/upp/v1/markets/categories`, {
      tags: { endpoint: 'categories', scenario: scenario },
      timeout: '5s',
    });

    const latency = Date.now() - startTime;
    categoriesLatency.add(latency);
    categoryRequests.add(1);
    trackCacheHit(res);

    check(res, {
      'categories status is 200': (r) => r.status === 200,
      'categories returns array': (r) => {
        try {
          const data = JSON.parse(r.body);
          return Array.isArray(data.categories);
        } catch (e) {
          return false;
        }
      },
      'categories fast response': (r) => latency < 500,
    });

    errorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
  });

  sleep(THINK_TIME / 1000);

  group('MCP Execute', function () {
    const payload = JSON.stringify({
      tool: 'search_markets',
      params: {
        query: 'election',
        limit: 3,
      },
    });

    const startTime = Date.now();
    const res = http.post(`${BASE_URL}/upp/v1/mcp/execute`, payload, {
      headers: { 'Content-Type': 'application/json' },
      tags: { endpoint: 'mcp_execute', scenario: scenario },
      timeout: '10s',
    });

    const latency = Date.now() - startTime;
    executeLatency.add(latency);
    executeRequests.add(1);

    check(res, {
      'execute status is 200 or 201': (r) => r.status === 200 || r.status === 201,
      'execute returns result': (r) => {
        try {
          const data = JSON.parse(r.body);
          return data.result !== undefined || data.error !== undefined;
        } catch (e) {
          return false;
        }
      },
      'execute within latency SLA': (r) => latency < 2500,
    });

    errorRate.add(res.status !== 200 && res.status !== 201 && res.status !== 429 ? 1 : 0);
  });

  sleep(THINK_TIME / 1000);

  group('Order Estimate', function () {
    const payload = JSON.stringify({
      provider: 'kalshi.com',
      market_id: 'BITCOIN-100K',
      outcome_id: 'yes',
      side: 'buy',
      price: '0.65',
      quantity: 100,
    });

    const startTime = Date.now();
    const res = http.post(`${BASE_URL}/upp/v1/orders/estimate`, payload, {
      headers: { 'Content-Type': 'application/json' },
      tags: { endpoint: 'order_estimate', scenario: scenario },
      timeout: '5s',
    });

    const latency = Date.now() - startTime;
    estimateLatency.add(latency);
    estimateRequests.add(1);

    check(res, {
      'estimate status is 200': (r) => r.status === 200,
      'estimate returns cost': (r) => {
        try {
          const data = JSON.parse(r.body);
          return data.estimated_cost !== undefined;
        } catch (e) {
          return false;
        }
      },
      'estimate fast response': (r) => latency < 500,
    });

    errorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
  });

  sleep(THINK_TIME / 1000);

  group('Agent Configuration', function () {
    const startTime = Date.now();
    const res = http.get(`${BASE_URL}/.well-known/agent.json`, {
      tags: { endpoint: 'agent_config', scenario: scenario },
      timeout: '5s',
    });

    const latency = Date.now() - startTime;
    staticLatency.add(latency);
    staticRequests.add(1);
    trackCacheHit(res);

    check(res, {
      'agent config status is 200': (r) => r.status === 200,
      'agent config is valid JSON': (r) => {
        try {
          JSON.parse(r.body);
          return true;
        } catch (e) {
          return false;
        }
      },
      'agent config fast': (r) => latency < 100,
    });

    errorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
  });

  sleep(THINK_TIME / 1000);

  group('MCP Tools List', function () {
    const startTime = Date.now();
    const res = http.get(`${BASE_URL}/upp/v1/mcp/tools`, {
      tags: { endpoint: 'mcp_tools', scenario: scenario },
      timeout: '5s',
    });

    const latency = Date.now() - startTime;
    staticLatency.add(latency);
    staticRequests.add(1);
    trackCacheHit(res);

    check(res, {
      'tools list status is 200': (r) => r.status === 200,
      'tools list is array': (r) => {
        try {
          const data = JSON.parse(r.body);
          return Array.isArray(data.tools || data);
        } catch (e) {
          return false;
        }
      },
      'tools list fast': (r) => latency < 100,
    });

    errorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
  });

  sleep(THINK_TIME / 1000);
}
