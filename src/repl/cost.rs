//! Session cost estimation — yoyo-style static rates via [`super::native_pricing`].

use {crate::providers::Provider, yoagent::types::Usage};

use super::native_pricing::{self, PerMTok};

fn resolve_pricing(model: &str) -> Option<PerMTok> {
    native_pricing::native_model_pricing(model)
}

/// Lines for `/model info` — static estimated rates (yoyo-style rules).
pub fn format_pricing_lines(_provider: Provider, model: &str) -> Vec<String> {
    native_pricing::format_native_pricing_lines(model)
}

pub fn cost_breakdown(
    usage: &Usage,
    _provider: Provider,
    model: &str,
) -> Option<(f64, f64, f64, f64)> {
    let (input_per_m, cache_write_per_m, cache_read_per_m, output_per_m) = resolve_pricing(model)?;

    let input_cost = usage.input as f64 * input_per_m / 1_000_000.0;
    let cache_write_cost = usage.cache_write as f64 * cache_write_per_m / 1_000_000.0;
    let cache_read_cost = usage.cache_read as f64 * cache_read_per_m / 1_000_000.0;
    let output_cost = usage.output as f64 * output_per_m / 1_000_000.0;

    Some((input_cost, cache_write_cost, cache_read_cost, output_cost))
}

#[allow(dead_code)]
pub fn estimate_cost(usage: &Usage, provider: Provider, model: &str) -> Option<f64> {
    let (input_cost, cw_cost, cr_cost, output_cost) = cost_breakdown(usage, provider, model)?;
    Some(input_cost + cw_cost + cr_cost + output_cost)
}
