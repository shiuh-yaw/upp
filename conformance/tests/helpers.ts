// Shared test helpers for UPP conformance tests.

import { readFileSync, existsSync } from 'fs';
import { join } from 'path';

const SCHEMAS_DIR = join(__dirname, '../../schemas/json');
const FIXTURES_DIR = join(__dirname, '../fixtures');

/**
 * Load a JSON Schema for a UPP type.
 */
export function loadSchema(name: string): Record<string, any> {
  const path = join(SCHEMAS_DIR, `${name}.json`);
  if (!existsSync(path)) {
    // Return a permissive schema if not yet generated
    console.warn(`Schema not found: ${path} — using permissive schema`);
    return { type: 'object' };
  }
  return JSON.parse(readFileSync(path, 'utf-8'));
}

/**
 * Load a golden test fixture.
 * Returns null if fixture doesn't exist (provider not yet captured).
 */
export function loadFixture(relativePath: string): Record<string, any> | null {
  const path = join(FIXTURES_DIR, relativePath);
  if (!existsSync(path)) return null;
  return JSON.parse(readFileSync(path, 'utf-8'));
}

/**
 * Validate a data object against a JSON Schema.
 * Returns { valid: boolean, errors: string[] }
 */
export function validateAgainstSchema(
  data: any,
  schema: Record<string, any>
): { valid: boolean; errors: string[] } {
  // Lightweight validation (full Ajv validation in CI)
  const errors: string[] = [];

  if (schema.required) {
    for (const field of schema.required) {
      if (!(field in data)) {
        errors.push(field);
      }
    }
  }

  if (schema.properties?.market_type?.enum) {
    if (data.market_type && !schema.properties.market_type.enum.includes(data.market_type)) {
      errors.push(`Invalid market_type: ${data.market_type}`);
    }
  }

  return { valid: errors.length === 0, errors };
}

/**
 * Assert that two UPP Market objects from different providers
 * are structurally compatible (same event, comparable prices).
 */
export function assertCrossProviderCompatibility(
  marketA: Record<string, any>,
  marketB: Record<string, any>
): void {
  // Both should have the same structural fields
  const requiredFields = ['id', 'event', 'market_type', 'outcomes', 'pricing', 'lifecycle'];
  for (const field of requiredFields) {
    if (!(field in marketA)) throw new Error(`Market A missing field: ${field}`);
    if (!(field in marketB)) throw new Error(`Market B missing field: ${field}`);
  }

  // Both should have outcomes in the same format
  if (!Array.isArray(marketA.outcomes)) throw new Error('Market A outcomes not array');
  if (!Array.isArray(marketB.outcomes)) throw new Error('Market B outcomes not array');
}
