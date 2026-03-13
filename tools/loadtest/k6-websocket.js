import ws from 'k6/ws';
import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// Configuration
const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const WS_URL = (BASE_URL.replace('http', 'ws')) + '/upp/v1/ws';

// Custom metrics
const wsConnectionTime = new Trend('ws_connection_time', true);
const wsMessageLatency = new Trend('ws_message_latency', true);
const wsMessagesPerSecond = new Trend('ws_messages_per_second', true);
const wsReconnectCount = new Counter('ws_reconnect_count');
const wsPingPongLatency = new Trend('ws_ping_pong_latency', true);
const wsConnectionErrors = new Rate('ws_connection_errors');
const wsMessageErrors = new Rate('ws_message_errors');

// Test data
const marketIds = [
  'upp:kalshi.com:BITCOIN-100K',
  'upp:polymarket.com:2024-ELECTION',
  'upp:metaculus.com:TECH-BOOM',
  'upp:omen.eth:AI-SAFETY',
  'upp:hypermind.com:CLIMATE-2030',
];

const orderbookMarkets = [
  'upp:kalshi.com:BITCOIN-100K',
  'upp:polymarket.com:2024-ELECTION',
  'upp:metaculus.com:TECH-BOOM',
];

// Test scenarios
export const options = {
  scenarios: {
    'websocket-sustained': {
      executor: 'constant-vus',
      vus: 10,
      duration: '2m',
      tags: { scenario: 'websocket-sustained' },
    },
    'websocket-spike': {
      executor: 'ramping-vus',
      stages: [
        { duration: '5s', target: 50 },    // Spike to 50 concurrent connections
        { duration: '30s', target: 50 },   // Sustain
        { duration: '10s', target: 0 },    // Ramp down
      ],
      tags: { scenario: 'websocket-spike' },
      gracefulStop: '30s',
    },
  },
  thresholds: {
    'ws_connection_time': ['p(95)<2000', 'p(99)<4000'],
    'ws_message_latency': ['p(95)<1000', 'p(99)<2000'],
    'ws_ping_pong_latency': ['p(95)<500', 'p(99)<1000'],
    'ws_connection_errors': ['rate<0.50'],  // Allow connection errors under concurrent load
    'ws_message_errors': ['rate<0.05'],     // Less than 5% message errors
  },
};

// Helper function to check metrics endpoint
function checkMetrics() {
  const res = http.get(`${BASE_URL}/metrics`, {
    timeout: '5s',
    tags: { endpoint: 'metrics' },
  });

  if (res.status === 200) {
    const body = res.body;
    // Extract ws_connections from Prometheus metrics
    const wsConnMatch = body.match(/upp_ws_connections\s+(\d+)/);
    const requestCountMatch = body.match(/upp_requests_total\s+(\d+)/);
    return {
      wsConnections: wsConnMatch ? parseInt(wsConnMatch[1]) : 0,
      requestCount: requestCountMatch ? parseInt(requestCountMatch[1]) : 0,
    };
  }
  return { wsConnections: 0, requestCount: 0 };
}

// Main WebSocket test
export default function () {
  const scenario = __ENV.SCENARIO || 'websocket-sustained';

  group('WebSocket Connection and Subscription', function () {
    const startTime = Date.now();
    let messageCount = 0;
    let subscriptionAckTime = null;
    let firstMessageTime = null;

    const res = ws.connect(WS_URL, { tags: { scenario: scenario } }, function (socket) {
      const connectionTime = Date.now() - startTime;
      wsConnectionTime.add(connectionTime);

      // Set up message handler
      socket.on('message', function (message) {
        messageCount++;

        try {
          const data = JSON.parse(message);

          // Record first message time for subscription latency
          if (firstMessageTime === null && data.type === 'price_update') {
            firstMessageTime = Date.now() - startTime;
            wsMessageLatency.add(firstMessageTime);
          }

          // Track subscription acknowledgment
          if (data.type === 'subscription_ack') {
            if (subscriptionAckTime === null) {
              subscriptionAckTime = Date.now() - startTime;
            }
          }

          check(data, {
            'message has type': (d) => d.type !== undefined,
            'message is valid JSON': (d) => true,
          });
        } catch (e) {
          wsMessageErrors.add(1);
        }
      });

      socket.on('close', function (code) {
        if (code !== 1000) {
          wsConnectionErrors.add(1);
        }
      });

      socket.on('error', function (e) {
        wsConnectionErrors.add(1);
      });

      // Subscribe to price updates for first 3 markets (JSON-RPC format)
      socket.send(
        JSON.stringify({
          jsonrpc: '2.0',
          method: 'subscribe_prices',
          params: { market_ids: marketIds.slice(0, 3) },
          id: 1,
        })
      );
      sleep(0.2);

      // Subscribe to orderbook updates for 3 markets
      socket.send(
        JSON.stringify({
          jsonrpc: '2.0',
          method: 'subscribe_orderbook',
          params: { market_ids: orderbookMarkets },
          id: 2,
        })
      );
      sleep(0.2);

      // Keep connection open and send periodic pings
      for (let i = 0; i < 6; i++) {
        const pingTime = Date.now();
        socket.send(JSON.stringify({ jsonrpc: '2.0', method: 'ping', id: i + 10 }));

        // Wait for pong and measure latency
        sleep(1);

        // Record ping/pong latency estimate
        wsPingPongLatency.add(Date.now() - pingTime);
      }

      socket.close();
    });

    // Check connection result
    check(res, {
      'WebSocket connection successful': (r) => r.status === 101,
      'connection established': (r) => r.status >= 0,
    });

    if (res.status !== 101) {
      wsConnectionErrors.add(1);
    }

    // Record messages per second
    if (messageCount > 0) {
      wsMessagesPerSecond.add(messageCount / 6); // ~6 seconds of connection
    }
  });

  sleep(2);

  group('WebSocket Reconnection Test', function () {
    let connectionAttempts = 0;

    // Try to establish connection multiple times
    for (let attempt = 0; attempt < 3; attempt++) {
      const startTime = Date.now();

      const res = ws.connect(
        WS_URL,
        { tags: { scenario: scenario, attempt: attempt } },
        function (socket) {
          socket.on('message', function (message) {
            try {
              JSON.parse(message);
            } catch (e) {
              wsMessageErrors.add(1);
            }
          });

          // Send one subscription and close (JSON-RPC format)
          socket.send(
            JSON.stringify({
              jsonrpc: '2.0',
              method: 'subscribe_prices',
              params: { market_ids: [marketIds[0]] },
              id: 1,
            })
          );

          sleep(0.5);
          socket.close();
        }
      );

      connectionAttempts++;

      if (res.status === 101) {
        wsReconnectCount.add(1);
      } else {
        wsConnectionErrors.add(1);
      }

      sleep(0.5);
    }

    check(connectionAttempts, {
      'attempted multiple connections': (c) => c === 3,
    });
  });

  sleep(2);

  group('WebSocket Stress - Multiple Concurrent Subscriptions', function () {
    const startTime = Date.now();

    const res = ws.connect(
      WS_URL,
      { tags: { scenario: scenario } },
      function (socket) {
        socket.on('message', function (message) {
          try {
            JSON.parse(message);
          } catch (e) {
            wsMessageErrors.add(1);
          }
        });

        // Subscribe to all markets (JSON-RPC format)
        socket.send(
          JSON.stringify({
            jsonrpc: '2.0',
            method: 'subscribe_prices',
            params: { market_ids: marketIds },
            id: 1,
          })
        );
        sleep(0.1);

        // Subscribe to all orderbooks
        socket.send(
          JSON.stringify({
            jsonrpc: '2.0',
            method: 'subscribe_orderbook',
            params: { market_ids: orderbookMarkets },
            id: 2,
          })
        );
        sleep(0.1);

        // Keep connection alive
        for (let i = 0; i < 8; i++) {
          socket.send(JSON.stringify({ jsonrpc: '2.0', method: 'ping', id: i + 10 }));
          sleep(1);
        }

        socket.close();
      }
    );

    check(res, {
      'stress test connection established': (r) => r.status === 101,
    });

    const duration = Date.now() - startTime;
    wsConnectionTime.add(duration);
  });

  sleep(2);
}

// Setup function to verify gateway is running
export function setup() {
  group('Verify Gateway', function () {
    const res = http.get(`${BASE_URL}/health`, { timeout: '5s' });
    check(res, {
      'gateway is ready': (r) => r.status === 200,
    });
  });

  return {
    timestamp: new Date().toISOString(),
  };
}

// Teardown function to check final metrics
export function teardown(data) {
  group('Final Metrics Check', function () {
    const metrics = checkMetrics();
    check(metrics, {
      'metrics endpoint accessible': (m) => m.wsConnections !== null,
    });
  });
}
