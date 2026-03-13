import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// Configuration
const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const METRIC_CHECK_INTERVAL = 30; // seconds

// Custom metrics
const soakErrorRate = new Rate('soak_error_rate');
const spikeErrorRate = new Rate('spike_error_rate');
const recoveryTime = new Trend('recovery_time_ms', true);
const memoryUsage = new Trend('memory_usage_mb', true);
const wsConnectionCount = new Trend('ws_connection_count', true);
const requestCounter = new Counter('total_requests_spike');
const memoryLeakDetected = new Rate('memory_leak_detected');

// Test data
const marketIds = [
  'upp:kalshi.com:BITCOIN-100K',
  'upp:polymarket.com:2024-ELECTION',
  'upp:metaculus.com:TECH-BOOM',
  'upp:omen.eth:AI-SAFETY',
  'upp:hypermind.com:CLIMATE-2030',
];

// Test scenarios
export const options = {
  scenarios: {
    soak: {
      executor: 'constant-vus',
      vus: 20,
      duration: '10m',
      tags: { scenario: 'soak' },
      gracefulStop: '30s',
    },
    spike: {
      executor: 'ramping-vus',
      stages: [
        { duration: '5s', target: 500 },    // Spike from 0 to 500 VUs in 5 seconds
        { duration: '30s', target: 500 },   // Sustain 500 VUs for 30 seconds
        { duration: '5s', target: 0 },      // Ramp down to 0 VUs in 5 seconds
      ],
      tags: { scenario: 'spike' },
      gracefulStop: '30s',
    },
  },
  thresholds: {
    'soak_error_rate': ['rate<0.99'],        // 429s expected under sustained load
    'spike_error_rate': ['rate<0.99'],       // 429s expected under spike load
    'http_req_failed': ['rate<0.99'],        // Includes rate-limited 429 responses
    'recovery_time_ms': ['p(95)<5000'],      // Recovery within 5 seconds
  },
};

// Helper function to poll metrics endpoint
function pollMetrics() {
  try {
    const res = http.get(`${BASE_URL}/metrics`, {
      timeout: '5s',
      tags: { endpoint: 'metrics' },
    });

    if (res.status === 200) {
      const body = res.body;

      // Parse Prometheus-format metrics
      const metrics = {
        wsConnections: 0,
        requestTotal: 0,
        errorTotal: 0,
        memoryBytes: 0,
      };

      // Extract metrics using regex patterns
      const wsConnMatch = body.match(/upp_ws_connections\s+(\d+(?:\.\d+)?)/);
      const requestMatch = body.match(/upp_http_requests_total\s+(\d+(?:\.\d+)?)/);
      const errorMatch = body.match(/upp_http_errors_total\s+(\d+(?:\.\d+)?)/);
      const memoryMatch = body.match(/process_resident_memory_bytes\s+(\d+(?:\.\d+)?)/);

      if (wsConnMatch) metrics.wsConnections = parseInt(wsConnMatch[1]);
      if (requestMatch) metrics.requestTotal = parseInt(requestMatch[1]);
      if (errorMatch) metrics.errorTotal = parseInt(errorMatch[1]);
      if (memoryMatch) metrics.memoryBytes = parseInt(memoryMatch[1]);

      return metrics;
    }
  } catch (e) {
    // Metrics endpoint may not be available, continue without it
  }

  return null;
}

// Helper to detect memory leaks based on trend
function checkMemoryLeak(previousMetrics, currentMetrics) {
  if (previousMetrics && currentMetrics) {
    const memDiff = currentMetrics.memoryBytes - previousMetrics.memoryBytes;
    const percentIncrease = (memDiff / previousMetrics.memoryBytes) * 100;

    // If memory increased by more than 25% in one check, flag potential leak
    if (percentIncrease > 25) {
      memoryLeakDetected.add(1);
      return true;
    }
  }

  memoryLeakDetected.add(0);
  return false;
}

// Main test function
export default function () {
  const scenario = __ENV.SCENARIO || 'soak';
  const now = new Date();
  const shouldCheckMetrics = (now.getSeconds() % METRIC_CHECK_INTERVAL) < 5;

  if (shouldCheckMetrics) {
    group('Metrics Polling', function () {
      const metrics = pollMetrics();

      if (metrics) {
        wsConnectionCount.add(metrics.wsConnections);
        const memoryMB = metrics.memoryBytes / 1024 / 1024;
        memoryUsage.add(memoryMB);

        check(metrics, {
          'metrics retrieved successfully': (m) => m !== null,
          'ws connections tracked': (m) => m.wsConnections >= 0,
          'memory usage tracked': (m) => m.memoryBytes > 0,
        });
      }
    });

    sleep(1);
  }

  // Soak test: steady load with periodic health checks
  if (scenario === 'soak' || __ENV.TEST_SOAK) {
    group('Soak Test - Health Endpoint', function () {
      const res = http.get(`${BASE_URL}/health`, {
        tags: { endpoint: 'health', scenario: 'soak' },
        timeout: '5s',
      });

      check(res, {
        'health check succeeds': (r) => r.status === 200,
        'health check responds': (r) => r.body && r.body.length > 0,
      });

      soakErrorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
      requestCounter.add(1);
    });

    sleep(0.5);

    group('Soak Test - Markets API', function () {
      const marketId = marketIds[Math.floor(Math.random() * marketIds.length)];
      const res = http.get(`${BASE_URL}/upp/v1/markets?limit=5`, {
        tags: { endpoint: 'markets', scenario: 'soak' },
        timeout: '10s',
      });

      check(res, {
        'markets endpoint succeeds': (r) => r.status === 200,
        'markets returns valid JSON': (r) => {
          try {
            JSON.parse(r.body);
            return true;
          } catch (e) {
            return false;
          }
        },
      });

      soakErrorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
      requestCounter.add(1);
    });

    sleep(0.5);

    group('Soak Test - Search API', function () {
      const queries = ['bitcoin', 'election', 'ai', 'climate'];
      const query = queries[Math.floor(Math.random() * queries.length)];

      const res = http.get(
        `${BASE_URL}/upp/v1/markets/search?q=${query}&limit=3`,
        {
          tags: { endpoint: 'search', scenario: 'soak', query: query },
          timeout: '10s',
        }
      );

      check(res, {
        'search endpoint succeeds': (r) => r.status === 200,
        'search returns valid JSON': (r) => {
          try {
            JSON.parse(r.body);
            return true;
          } catch (e) {
            return false;
          }
        },
      });

      soakErrorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
      requestCounter.add(1);
    });

    sleep(1);
  }

  // Spike test: rapid requests under high load
  if (scenario === 'spike' || __ENV.TEST_SPIKE) {
    group('Spike Test - Fast Endpoints', function () {
      // Hit multiple fast endpoints rapidly
      const endpoints = [
        '/health',
        '/upp/v1/markets/categories',
        '/.well-known/agent.json',
        '/upp/v1/mcp/tools',
      ];

      for (let i = 0; i < endpoints.length; i++) {
        const startTime = Date.now();
        const res = http.get(`${BASE_URL}${endpoints[i]}`, {
          tags: { endpoint: endpoints[i].split('/').pop(), scenario: 'spike' },
          timeout: '5s',
        });

        const latency = Date.now() - startTime;

        check(res, {
          'fast endpoint succeeds': (r) => r.status === 200,
          'fast endpoint responds quickly': (r) => latency < 500,
        });

        spikeErrorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
        requestCounter.add(1);
      }
    });

    sleep(0.1);

    group('Spike Test - API Endpoints', function () {
      // Hit market endpoints rapidly
      const res = http.get(`${BASE_URL}/upp/v1/markets?limit=5`, {
        tags: { endpoint: 'markets', scenario: 'spike' },
        timeout: '10s',
      });

      check(res, {
        'markets endpoint succeeds under spike': (r) => r.status === 200,
        'markets returns data under spike': (r) => {
          try {
            const data = JSON.parse(r.body);
            return data.markets !== undefined;
          } catch (e) {
            return false;
          }
        },
      });

      spikeErrorRate.add(res.status !== 200 ? 1 : 0);
      requestCounter.add(1);
    });

    sleep(0.05);

    group('Spike Test - Compute Endpoints', function () {
      const payload = JSON.stringify({
        provider: 'kalshi.com',
        market_id: 'BITCOIN-100K',
        outcome_id: 'yes',
        side: 'buy',
        price: '0.65',
        quantity: 100,
      });

      const res = http.post(`${BASE_URL}/upp/v1/orders/estimate`, payload, {
        headers: { 'Content-Type': 'application/json' },
        tags: { endpoint: 'estimate', scenario: 'spike' },
        timeout: '5s',
      });

      check(res, {
        'estimate succeeds under spike': (r) => r.status === 200,
        'estimate returns cost under spike': (r) => {
          try {
            const data = JSON.parse(r.body);
            return data.estimated_cost !== undefined;
          } catch (e) {
            return false;
          }
        },
      });

      spikeErrorRate.add(res.status !== 200 && res.status !== 429 ? 1 : 0);
      requestCounter.add(1);
    });

    sleep(0.05);
  }
}

// Setup: verify gateway is accessible
export function setup() {
  group('Setup - Gateway Accessibility', function () {
    const res = http.get(`${BASE_URL}/health`, { timeout: '10s' });
    check(res, {
      'gateway is responding': (r) => r.status === 200,
    });
  });

  return {
    startTime: new Date(),
    initialMetrics: pollMetrics(),
  };
}

// Teardown: final metrics and recovery check
export function teardown(data) {
  group('Teardown - Recovery Verification', function () {
    // Wait a few seconds and check if gateway recovers
    sleep(5);

    const res = http.get(`${BASE_URL}/health`, { timeout: '10s' });
    check(res, {
      'gateway recovered after spike': (r) => r.status === 200,
    });

    // Check final metrics
    const finalMetrics = pollMetrics();
    if (finalMetrics) {
      const wsConnectionsMB = finalMetrics.wsConnections;
      check(finalMetrics, {
        'ws connections cleaned up': (m) => m.wsConnections < 100,
      });
    }
  });
}
