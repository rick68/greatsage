//! Normalize vendor-prefixed model ids (`vendor/model`, `:free`, `~alias`) for static pricing lookup.

/// Vendor slugs from the OpenRouter model-id catalog (`vendor/model-name`).
pub const VENDOR_SLUGS: &[&str] = &[
    "ai21",
    "aion-labs",
    "allenai",
    "amazon",
    "anthracite-org",
    "anthropic",
    "arcee-ai",
    "baidu",
    "bytedance-seed",
    "bytedance",
    "cerebras",
    "cognitivecomputations",
    "cohere",
    "deepcogito",
    "deepseek",
    "google",
    "groq",
    "gryphe",
    "ibm-granite",
    "inception",
    "inclusionai",
    "inflection",
    "kwaipilot",
    "liquid",
    "mancer",
    "meta-llama",
    "microsoft",
    "minimax",
    "mistralai",
    "moonshotai",
    "morph",
    "nex-agi",
    "nousresearch",
    "nvidia",
    "openai",
    "openrouter",
    "perceptron",
    "perplexity",
    "poolside",
    "qwen",
    "rekaai",
    "relace",
    "sakana",
    "sao10k",
    "stepfun",
    "switchpoint",
    "tencent",
    "thedrummer",
    "undi95",
    "upstage",
    "writer",
    "x-ai",
    "xiaomi",
    "z-ai",
];

/// Canonical model name after stripping vendor slug, variant suffix, and alias.
pub fn canonical_model_name(model_id: &str) -> String {
    let base = model_id.split(':').next().unwrap_or(model_id);
    let name = base.strip_prefix('~').unwrap_or(base).to_lowercase();
    if let Some((vendor, rest)) = name.split_once('/')
        && VENDOR_SLUGS.contains(&vendor)
    {
        return String::from(rest);
    }
    name
}
