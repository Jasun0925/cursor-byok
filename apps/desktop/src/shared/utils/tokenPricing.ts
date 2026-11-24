import type { ModelTokenPricing, ModelUsage, OverviewMetrics, TokenPricingSettings } from "../api";

export type TokenCosts = {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
};

export function pricingForModel(
  modelHash: string | null,
  global: TokenPricingSettings,
  overrides: ModelTokenPricing[],
): TokenPricingSettings {
  const override = modelHash === null
    ? undefined
    : overrides.find((entry) => entry.model_hash === modelHash);
  return override ?? global;
}

export function costsForMetrics(
  metrics: OverviewMetrics,
  pricing: TokenPricingSettings,
): TokenCosts {
  return {
    input: metrics.input_tokens * pricing.input_per_million / 1_000_000,
    output: metrics.output_tokens * pricing.output_per_million / 1_000_000,
    cacheRead: metrics.cache_read_tokens * pricing.cache_read_per_million / 1_000_000,
    cacheWrite: metrics.cache_write_tokens * pricing.cache_write_per_million / 1_000_000,
  };
}

export function costsForUsage(
  row: ModelUsage,
  global: TokenPricingSettings,
  overrides: ModelTokenPricing[],
): TokenCosts {
  return costsForMetrics(row.metrics, pricingForModel(row.model_hash, global, overrides));
}

export function aggregateUsageCosts(
  rows: ModelUsage[],
  global: TokenPricingSettings,
  overrides: ModelTokenPricing[],
): TokenCosts {
  return rows.reduce((total, row) => {
    const costs = costsForUsage(row, global, overrides);
    return {
      input: total.input + costs.input,
      output: total.output + costs.output,
      cacheRead: total.cacheRead + costs.cacheRead,
      cacheWrite: total.cacheWrite + costs.cacheWrite,
    };
  }, { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 });
}
