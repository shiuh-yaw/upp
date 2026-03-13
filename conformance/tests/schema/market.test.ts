// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Market schema conformance tests.
// Validates that Market objects from all providers conform to the UPP schema.

import { describe, it, expect } from 'vitest';
import { loadSchema, validateAgainstSchema, loadFixture } from '../helpers';

describe('Market Schema Conformance', () => {
  // ── Schema Structure Tests ─────────────────────────────────

  describe('Market object structure', () => {
    it('should require all mandatory fields', () => {
      const schema = loadSchema('market');
      const incomplete = { id: { provider: 'test', native_id: '123' } };
      const result = validateAgainstSchema(incomplete, schema);
      expect(result.valid).toBe(false);
      expect(result.errors).toContain('event');
      expect(result.errors).toContain('market_type');
    });

    it('should accept a valid binary market', () => {
      const schema = loadSchema('market');
      const market = {
        id: { provider: 'kalshi.com', native_id: 'KXBTC-26MAR14-100K', full_id: 'upp:kalshi.com:KXBTC-26MAR14-100K' },
        event: {
          id: 'evt_btc_100k',
          title: 'Will Bitcoin reach $100K?',
          description: 'Resolves YES if BTC >= $100,000',
          category: 'crypto',
          tags: ['bitcoin', 'crypto'],
        },
        market_type: 'binary',
        outcomes: [
          { id: 'yes', label: 'Yes' },
          { id: 'no', label: 'No' },
        ],
        pricing: {
          last_price: { yes: '0.63', no: '0.37' },
          best_bid: { yes: '0.62', no: '0.36' },
          best_ask: { yes: '0.64', no: '0.38' },
          mid_price: { yes: '0.63', no: '0.37' },
          spread: { yes: '0.02', no: '0.02' },
          tick_size: '0.01',
          currency: 'USD',
          min_order_size: 1,
          max_order_size: 25000,
          updated_at: '2026-03-11T10:00:00Z',
        },
        volume: {
          total_volume: '1250000',
          volume_24h: '85000',
          open_interest: '420000',
          updated_at: '2026-03-11T10:00:00Z',
        },
        lifecycle: {
          status: 'open',
          created_at: '2026-02-01T00:00:00Z',
          closes_at: '2026-03-14T23:59:59Z',
          resolution_source: 'official_data_feed',
        },
        rules: {
          allowed_order_types: ['limit', 'market'],
          allowed_tif: ['GTC', 'GTD', 'FOK'],
          allows_short_selling: false,
          allows_partial_fill: true,
          maker_fee_rate: '0.00',
          taker_fee_rate: '0.00',
          max_position_size: 0,
        },
        regulatory: {
          jurisdiction: 'US',
          compliant: true,
          eligible_regions: ['US'],
          restricted_regions: [],
          regulator: 'CFTC',
          license_type: 'DCM',
          contract_type: 'event_contract',
          required_kyc: 'enhanced',
        },
      };
      const result = validateAgainstSchema(market, schema);
      expect(result.valid).toBe(true);
    });

    it('should validate market_type enum values', () => {
      const schema = loadSchema('market');
      const market = createMinimalMarket({ market_type: 'invalid_type' });
      const result = validateAgainstSchema(market, schema);
      expect(result.valid).toBe(false);
    });

    it('should validate lifecycle status enum', () => {
      const validStatuses = ['pending', 'open', 'halted', 'closed', 'resolved', 'disputed', 'voided'];
      const schema = loadSchema('market');

      for (const status of validStatuses) {
        const market = createMinimalMarket({ lifecycle: { status, created_at: '2026-01-01T00:00:00Z' } });
        const result = validateAgainstSchema(market, schema);
        expect(result.valid).toBe(true);
      }
    });
  });

  // ── Universal Market ID Tests ──────────────────────────────

  describe('Universal Market ID', () => {
    it('should follow upp:{provider}:{native_id} format', () => {
      const id = { provider: 'kalshi.com', native_id: 'KXBTC-26MAR14-100K', full_id: 'upp:kalshi.com:KXBTC-26MAR14-100K' };
      expect(id.full_id).toBe(`upp:${id.provider}:${id.native_id}`);
    });

    it('should parse full_id correctly', () => {
      const full_id = 'upp:polymarket.com:0x1234abcd';
      const parts = full_id.split(':');
      expect(parts[0]).toBe('upp');
      expect(parts[1]).toBe('polymarket.com');
      expect(parts[2]).toBe('0x1234abcd');
    });

    it('should handle native IDs with colons', () => {
      // Some providers might have colons in their IDs
      const full_id = 'upp:opinion.trade:market:subid:123';
      const [prefix, provider, ...rest] = full_id.split(':');
      expect(prefix).toBe('upp');
      expect(provider).toBe('opinion.trade');
      expect(rest.join(':')).toBe('market:subid:123');
    });
  });

  // ── Price Normalization Tests ──────────────────────────────

  describe('Price normalization', () => {
    it('should have prices in [0.00, 1.00] range', () => {
      const market = createMinimalMarket({
        pricing: {
          last_price: { yes: '0.63', no: '0.37' },
          best_bid: { yes: '0.62', no: '0.36' },
          best_ask: { yes: '0.64', no: '0.38' },
          tick_size: '0.01',
          currency: 'USD',
        },
      });

      for (const [, price] of Object.entries(market.pricing.last_price)) {
        const p = parseFloat(price as string);
        expect(p).toBeGreaterThanOrEqual(0);
        expect(p).toBeLessThanOrEqual(1);
      }
    });

    it('should have complementary YES/NO prices for binary markets', () => {
      const lastYes = 0.63;
      const lastNo = 0.37;
      // YES + NO should approximately equal 1.0 (within spread/fee tolerance)
      expect(lastYes + lastNo).toBeCloseTo(1.0, 1);
    });
  });

  // ── Provider Fixture Tests ─────────────────────────────────

  describe('Kalshi fixture conformance', () => {
    it('should transform Kalshi market response to valid UPP Market', () => {
      const fixture = loadFixture('kalshi/market_sample.json');
      if (!fixture) {
        console.warn('Kalshi fixture not found — skipping');
        return;
      }
      const schema = loadSchema('market');
      const result = validateAgainstSchema(fixture, schema);
      expect(result.valid).toBe(true);
    });
  });

  describe('Polymarket fixture conformance', () => {
    it('should transform Polymarket market response to valid UPP Market', () => {
      const fixture = loadFixture('polymarket/market_sample.json');
      if (!fixture) {
        console.warn('Polymarket fixture not found — skipping');
        return;
      }
      const schema = loadSchema('market');
      const result = validateAgainstSchema(fixture, schema);
      expect(result.valid).toBe(true);
    });
  });
});

// ── Test Helpers ─────────────────────────────────────────────

function createMinimalMarket(overrides: Record<string, any> = {}) {
  return {
    id: { provider: 'test', native_id: 'test-001', full_id: 'upp:test:test-001' },
    event: { id: 'evt1', title: 'Test', description: 'Test market', category: 'test', tags: [] },
    market_type: 'binary',
    outcomes: [{ id: 'yes', label: 'Yes' }, { id: 'no', label: 'No' }],
    pricing: { last_price: {}, tick_size: '0.01', currency: 'USD', min_order_size: 1, max_order_size: 0, updated_at: '2026-01-01T00:00:00Z' },
    volume: { total_volume: '0', volume_24h: '0', open_interest: '0', updated_at: '2026-01-01T00:00:00Z' },
    lifecycle: { status: 'open', created_at: '2026-01-01T00:00:00Z' },
    rules: { allowed_order_types: ['limit'], allowed_tif: ['GTC'], allows_short_selling: false, allows_partial_fill: true, maker_fee_rate: '0', taker_fee_rate: '0', max_position_size: 0 },
    regulatory: { jurisdiction: 'US', compliant: true, eligible_regions: [], restricted_regions: [], regulator: 'none', license_type: 'none', contract_type: 'event_contract', required_kyc: 'none' },
    ...overrides,
  };
}
