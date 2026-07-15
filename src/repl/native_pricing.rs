//! Static per-model rates (per MTok).
//!
//! Maintain pricing in two places:
//! - [`rates`] — dollar amounts (per MTok)
//! - classify helpers — model id → [`PriceTier`]
//!
//! yoyo-evolve rules for core vendors; extra tiers for greatsage / OpenRouter catalog.
//!
//! **Dual source:** UI paths (`/cost`, `/model info`) read this book. At agent install,
//! `ModelConfig.cost` MUST stay aligned for known models (`coding::model_config_for`
//! fills zero costs from here, and yoagent named presets carry the same rates). Unknown
//! models intentionally stay unpriced (`None` / all-zero cost) — never invent free.

use super::model_id::canonical_model_name;

/// `(input_per_MTok, cache_write_per_MTok, cache_read_per_MTok, output_per_MTok)`.
pub type PerMTok = (f64, f64, f64, f64);

/// Canonical price tier — one row in the rate book.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PriceTier {
    // Anthropic (current-gen tiers align with yoagent 0.13 ModelConfig presets)
    AnthropicFable5,
    AnthropicOpus48,
    AnthropicOpus45,
    AnthropicOpusLegacy,
    AnthropicSonnet5,
    AnthropicSonnet,
    AnthropicHaiku45,
    AnthropicHaikuLegacy,
    // OpenAI
    OpenAi41Mini,
    OpenAi41Nano,
    OpenAi41,
    OpenAi4oMini,
    OpenAi4o,
    OpenAiCodexMini,
    OpenAiO4Mini,
    OpenAiO3Mini,
    OpenAiO3,
    OpenAiO3Pro,
    OpenAiO1Pro,
    OpenAiO1,
    OpenAiGptAudio,
    OpenAiGptMiniLatest,
    OpenAiGptLatest,
    OpenAiGpt55Mini,
    OpenAiGpt55Pro,
    OpenAiGpt55,
    OpenAiGpt5Mini,
    OpenAiGpt5Pro,
    OpenAiGpt5,
    OpenAiGptOssLarge,
    OpenAiGptOssSmall,
    OpenAiGpt4Turbo,
    OpenAiGpt4Legacy,
    OpenAiGpt35,
    // Google
    GoogleDeepResearch,
    GoogleGemini3Pro,
    GoogleGemini3Flash,
    GoogleGemini25Pro,
    GoogleGemini25Flash,
    GoogleGemini20Flash,
    GoogleGeminiPro,
    GoogleGeminiFlash,
    GoogleGemmaLarge,
    GoogleGemmaSmall,
    GoogleLyria,
    // DeepSeek
    DeepSeekReasoner,
    DeepSeekChat,
    // Mistral
    MistralLarge,
    MistralMedium,
    MistralSmall,
    MistralCodestral,
    MistralNemo,
    MistralMixtral8x7,
    MistralDefault,
    // xAI
    XaiGrok4Mini,
    XaiGrok4,
    XaiGrok3Mini,
    XaiGrok3,
    XaiGrok2,
    XaiGrokDefault,
    // ZAI
    ZaiGlmPremium,
    ZaiGlmBudget,
    ZaiGlmLong,
    // MiniMax
    MiniMaxM3,
    MiniMaxM27,
    MiniMaxDefault,
    // Open-weight & hosted families
    QwenMax,
    QwenPlus,
    QwenFlash,
    QwenDefault,
    KimiLarge,
    KimiDefault,
    CoherePremium,
    CohereStandard,
    NovaPremier,
    NovaLite,
    NovaMicro,
    SonarPro,
    SonarBasic,
    NemotronLarge,
    NemotronDefault,
    GroqCompound,
    Llama70b,
    Llama8b,
    LlamaGuard,
    PhiMini,
    PhiDefault,
    MidTier90,
    MidTier30,
    MidTier80,
    MidTier60,
    MidTier10,
    Budget20,
    Budget10,
    Budget05,
    OpenRouterMeta,
}

mod rates {
    use super::PerMTok;

    pub const fn nc(input: f64, output: f64) -> PerMTok {
        (input, 0.0, 0.0, output)
    }

    /// yoagent `claude_fable_5` CostConfig
    pub const ANTHROPIC_FABLE_5: PerMTok = (10.0, 12.5, 1.0, 50.0);
    /// yoagent `claude_opus_4_8` CostConfig
    pub const ANTHROPIC_OPUS_48: PerMTok = (5.0, 6.25, 0.50, 25.0);
    pub const ANTHROPIC_OPUS_45: PerMTok = (5.0, 6.25, 0.50, 25.0);
    pub const ANTHROPIC_OPUS_LEGACY: PerMTok = (15.0, 18.75, 1.50, 75.0);
    /// yoagent `claude_sonnet_5` CostConfig
    pub const ANTHROPIC_SONNET_5: PerMTok = (3.0, 3.75, 0.30, 15.0);
    pub const ANTHROPIC_SONNET: PerMTok = (3.0, 3.75, 0.30, 15.0);
    /// yoagent `claude_haiku_4_5` CostConfig
    pub const ANTHROPIC_HAIKU_45: PerMTok = (1.0, 1.25, 0.10, 5.0);
    pub const ANTHROPIC_HAIKU_LEGACY: PerMTok = (0.80, 1.0, 0.08, 4.0);
}

fn tier_rates(tier: PriceTier) -> PerMTok {
    use PriceTier::*;
    use rates::{nc, *};

    match tier {
        AnthropicFable5 => ANTHROPIC_FABLE_5,
        AnthropicOpus48 => ANTHROPIC_OPUS_48,
        AnthropicOpus45 => ANTHROPIC_OPUS_45,
        AnthropicOpusLegacy => ANTHROPIC_OPUS_LEGACY,
        AnthropicSonnet5 => ANTHROPIC_SONNET_5,
        AnthropicSonnet => ANTHROPIC_SONNET,
        AnthropicHaiku45 => ANTHROPIC_HAIKU_45,
        AnthropicHaikuLegacy => ANTHROPIC_HAIKU_LEGACY,

        OpenAi41Mini => nc(0.40, 1.60),
        OpenAi41Nano => nc(0.10, 0.40),
        OpenAi41 => nc(2.00, 8.00),
        OpenAi4oMini => nc(0.15, 0.60),
        OpenAi4o => nc(2.50, 10.00),
        OpenAiCodexMini => nc(0.40, 1.60),
        OpenAiO4Mini => nc(1.10, 4.40),
        OpenAiO3Mini => nc(1.10, 4.40),
        OpenAiO3 => nc(2.00, 8.00),
        OpenAiO3Pro => nc(10.00, 40.00),
        OpenAiO1Pro => nc(20.00, 80.00),
        OpenAiO1 => nc(15.00, 60.00),
        OpenAiGptAudio => nc(2.50, 10.00),
        OpenAiGptMiniLatest => nc(0.40, 1.60),
        OpenAiGptLatest => nc(2.00, 8.00),
        OpenAiGpt55Mini => nc(0.40, 1.60),
        OpenAiGpt55Pro => nc(5.00, 20.00),
        OpenAiGpt55 => nc(5.00, 20.00),
        OpenAiGpt5Mini => nc(0.40, 1.60),
        OpenAiGpt5Pro => nc(5.00, 20.00),
        OpenAiGpt5 => nc(2.00, 8.00),
        OpenAiGptOssLarge => nc(0.15, 0.60),
        OpenAiGptOssSmall => nc(0.05, 0.20),
        OpenAiGpt4Turbo => nc(10.00, 30.00),
        OpenAiGpt4Legacy => nc(30.00, 60.00),
        OpenAiGpt35 => nc(0.50, 1.50),

        GoogleDeepResearch => nc(2.50, 15.00),
        GoogleGemini3Pro => nc(1.25, 10.00),
        GoogleGemini3Flash => nc(0.15, 0.60),
        GoogleGemini25Pro => nc(1.25, 10.00),
        GoogleGemini25Flash => nc(0.15, 0.60),
        GoogleGemini20Flash => nc(0.10, 0.40),
        GoogleGeminiPro => nc(1.25, 10.00),
        GoogleGeminiFlash => nc(0.15, 0.60),
        GoogleGemmaLarge => nc(0.10, 0.30),
        GoogleGemmaSmall => nc(0.05, 0.15),
        GoogleLyria => nc(0.30, 1.20),

        DeepSeekReasoner => nc(0.55, 2.19),
        DeepSeekChat => nc(0.27, 1.10),

        MistralLarge => nc(2.00, 6.00),
        MistralMedium => nc(1.00, 3.00),
        MistralSmall => nc(0.10, 0.30),
        MistralCodestral => nc(0.30, 0.90),
        MistralNemo => nc(0.15, 0.45),
        MistralMixtral8x7 => nc(0.24, 0.24),
        MistralDefault => nc(1.00, 3.00),

        XaiGrok4Mini => nc(0.60, 3.00),
        XaiGrok4 => nc(3.00, 15.00),
        XaiGrok3Mini => nc(0.30, 0.50),
        XaiGrok3 => nc(3.00, 15.00),
        XaiGrok2 => nc(2.00, 10.00),
        XaiGrokDefault => nc(3.00, 15.00),

        ZaiGlmPremium => nc(0.70, 0.70),
        ZaiGlmBudget => nc(0.01, 0.01),
        ZaiGlmLong => nc(0.14, 0.14),

        MiniMaxM3 => nc(0.30, 1.20),
        MiniMaxM27 => nc(0.20, 0.80),
        MiniMaxDefault => nc(0.15, 0.60),

        QwenMax => nc(0.60, 2.40),
        QwenPlus => nc(0.30, 1.20),
        QwenFlash => nc(0.05, 0.20),
        QwenDefault => nc(0.20, 0.80),

        KimiLarge => nc(0.60, 2.40),
        KimiDefault => nc(0.30, 1.20),

        CoherePremium => nc(2.50, 10.00),
        CohereStandard => nc(0.50, 1.50),

        NovaPremier => nc(0.80, 3.20),
        NovaLite => nc(0.06, 0.24),
        NovaMicro => nc(0.04, 0.16),

        SonarPro => nc(3.00, 15.00),
        SonarBasic => nc(1.00, 1.00),

        NemotronLarge => nc(0.60, 2.40),
        NemotronDefault => nc(0.20, 0.80),

        GroqCompound => nc(0.59, 0.79),
        Llama70b => nc(0.59, 0.79),
        Llama8b => nc(0.05, 0.08),
        LlamaGuard => nc(0.01, 0.01),

        PhiMini => nc(0.05, 0.15),
        PhiDefault => nc(0.10, 0.30),

        MidTier90 => nc(0.30, 0.90),
        MidTier30 => nc(0.30, 1.20),
        MidTier80 => nc(0.20, 0.80),
        MidTier60 => nc(0.15, 0.60),
        MidTier10 => nc(0.10, 0.30),

        Budget20 => nc(0.20, 0.80),
        Budget10 => nc(0.10, 0.30),
        Budget05 => nc(0.05, 0.20),

        OpenRouterMeta => nc(1.00, 3.00),
    }
}

/// Exact model id → tier overrides (checked before pattern rules).
const EXACT_TIERS: &[(&str, PriceTier)] = &[
    ("o3", PriceTier::OpenAiO3),
    ("auto", PriceTier::OpenRouterMeta),
    ("free", PriceTier::OpenRouterMeta),
    ("fusion", PriceTier::OpenRouterMeta),
    ("bodybuilder", PriceTier::OpenRouterMeta),
    ("owl-alpha", PriceTier::OpenRouterMeta),
    ("pareto-code", PriceTier::OpenRouterMeta),
];

fn classify_anthropic(model: &str) -> Option<PriceTier> {
    use PriceTier::*;
    // Fable before sonnet — must not fall through to AnthropicSonnet rates.
    if model.contains("fable") {
        return Some(AnthropicFable5);
    }
    if model.contains("opus") {
        if model.contains("4-8") || model.contains("4.8") {
            return Some(AnthropicOpus48);
        }
        if model.contains("4-5")
            || model.contains("4-6")
            || model.contains("4-7")
            || model.contains("4.5")
            || model.contains("4.6")
            || model.contains("4.7")
        {
            return Some(AnthropicOpus45);
        }
        return Some(AnthropicOpusLegacy);
    }
    if model.contains("sonnet") {
        // Sonnet 5 (not 3.5 / 4.x)
        if (model.contains("sonnet-5") || model.contains("sonnet_5") || model.contains("sonnet5"))
            && !model.contains("3-5")
            && !model.contains("3.5")
        {
            return Some(AnthropicSonnet5);
        }
        return Some(AnthropicSonnet);
    }
    if model.contains("haiku") {
        if model.contains("4-5") || model.contains("4.5") {
            return Some(AnthropicHaiku45);
        }
        return Some(AnthropicHaikuLegacy);
    }
    None
}

fn classify_openai(model: &str) -> Option<PriceTier> {
    use PriceTier::*;
    if model.starts_with("gpt-4.1") {
        if model.contains("mini") {
            return Some(OpenAi41Mini);
        }
        if model.contains("nano") {
            return Some(OpenAi41Nano);
        }
        return Some(OpenAi41);
    }
    if model.starts_with("gpt-4o") {
        if model.contains("mini") {
            return Some(OpenAi4oMini);
        }
        return Some(OpenAi4o);
    }
    if model.contains("codex-mini") {
        return Some(OpenAiCodexMini);
    }
    if model.starts_with("o4-mini") {
        return Some(OpenAiO4Mini);
    }
    if model.starts_with("o3-mini") {
        return Some(OpenAiO3Mini);
    }
    if model.starts_with("o3") {
        if model.contains("pro") {
            return Some(OpenAiO3Pro);
        }
        return Some(OpenAiO3);
    }
    if model.starts_with("o1") {
        if model.contains("pro") {
            return Some(OpenAiO1Pro);
        }
        return Some(OpenAiO1);
    }
    if model.contains("gpt-audio") {
        return Some(OpenAiGptAudio);
    }
    if model.contains("gpt-mini-latest") {
        return Some(OpenAiGptMiniLatest);
    }
    if model.contains("gpt-latest") || model.contains("gpt-chat-latest") {
        return Some(OpenAiGptLatest);
    }
    if model.starts_with("gpt-5.5") {
        if model.contains("mini") || model.contains("nano") {
            return Some(OpenAiGpt55Mini);
        }
        if model.contains("pro") {
            return Some(OpenAiGpt55Pro);
        }
        return Some(OpenAiGpt55);
    }
    if model.starts_with("gpt-5") {
        if model.contains("mini") || model.contains("nano") {
            return Some(OpenAiGpt5Mini);
        }
        if model.contains("pro") {
            return Some(OpenAiGpt5Pro);
        }
        return Some(OpenAiGpt5);
    }
    if model.contains("gpt-oss") {
        if model.contains("120") || model.contains("safeguard") {
            return Some(OpenAiGptOssLarge);
        }
        return Some(OpenAiGptOssSmall);
    }
    if model.contains("gpt-4") {
        if model.contains("turbo") {
            return Some(OpenAiGpt4Turbo);
        }
        return Some(OpenAiGpt4Legacy);
    }
    if model.contains("gpt-3.5") {
        return Some(OpenAiGpt35);
    }
    None
}

fn classify_google(model: &str) -> Option<PriceTier> {
    use PriceTier::*;
    if model.contains("deep-research") || model.contains("antigravity") {
        return Some(GoogleDeepResearch);
    }
    if model.contains("gemini-3") || model.contains("gemini-3.") {
        if model.contains("pro") {
            return Some(GoogleGemini3Pro);
        }
        return Some(GoogleGemini3Flash);
    }
    if model.contains("gemini-2.5-pro") || model.contains("computer-use") {
        return Some(GoogleGemini25Pro);
    }
    if model.contains("gemini-2.5-flash") {
        return Some(GoogleGemini25Flash);
    }
    if model.contains("gemini-2.0-flash") {
        return Some(GoogleGemini20Flash);
    }
    if model.contains("gemini") {
        if model.contains("pro") {
            return Some(GoogleGeminiPro);
        }
        return Some(GoogleGeminiFlash);
    }
    if model.contains("gemma") {
        if model.contains("27") || model.contains("31") || model.contains("26") {
            return Some(GoogleGemmaLarge);
        }
        return Some(GoogleGemmaSmall);
    }
    if model.contains("lyria") {
        return Some(GoogleLyria);
    }
    None
}

fn classify_deepseek(model: &str) -> Option<PriceTier> {
    use PriceTier::*;
    if model.contains("deepseek") {
        if model.contains("reasoner")
            || model.contains("r1")
            || model.contains("r2")
            || model.contains("v4-pro")
        {
            return Some(DeepSeekReasoner);
        }
        return Some(DeepSeekChat);
    }
    None
}

fn classify_mistral(model: &str) -> Option<PriceTier> {
    use PriceTier::*;
    if model.contains("mistral-large") || model.contains("mixtral-8x22b") {
        return Some(MistralLarge);
    }
    if model.contains("mistral-medium") {
        return Some(MistralMedium);
    }
    if model.contains("mistral-small")
        || model.contains("mistral-latest")
        || model.contains("ministral")
        || model.contains("voxtral")
    {
        return Some(MistralSmall);
    }
    if model.contains("codestral") || model.contains("devstral") {
        return Some(MistralCodestral);
    }
    if model.contains("mistral-nemo") || model.contains("mistral-saba") {
        return Some(MistralNemo);
    }
    if model.contains("mixtral") {
        return Some(MistralMixtral8x7);
    }
    if model.contains("mistral") {
        return Some(MistralDefault);
    }
    None
}

fn classify_xai(model: &str) -> Option<PriceTier> {
    use PriceTier::*;
    if model.contains("grok-4") {
        if model.contains("mini") || model.contains("build") {
            return Some(XaiGrok4Mini);
        }
        return Some(XaiGrok4);
    }
    if model.contains("grok-3") {
        if model.contains("mini") {
            return Some(XaiGrok3Mini);
        }
        return Some(XaiGrok3);
    }
    if model.contains("grok-2") {
        return Some(XaiGrok2);
    }
    if model.contains("grok") {
        return Some(XaiGrokDefault);
    }
    None
}

fn classify_zai(model: &str) -> Option<PriceTier> {
    use PriceTier::*;
    if model.contains("glm") || model.contains("zai-glm") {
        if model.contains("air") || model.contains("flash") || model.contains("phone") {
            return Some(ZaiGlmBudget);
        }
        if model.contains("long") {
            return Some(ZaiGlmLong);
        }
        return Some(ZaiGlmPremium);
    }
    None
}

fn classify_minimax(model: &str) -> Option<PriceTier> {
    use PriceTier::*;
    if model.contains("minimax") {
        if model.contains("m3") {
            return Some(MiniMaxM3);
        }
        if model.contains("m2.7") || model.contains("m2.5") {
            return Some(MiniMaxM27);
        }
        return Some(MiniMaxDefault);
    }
    None
}

fn classify_open_catalog(model: &str) -> Option<PriceTier> {
    use PriceTier::*;
    if model.contains("qwen") {
        if model.contains("max") || model.contains("235b") || model.contains("397b") {
            return Some(QwenMax);
        }
        if model.contains("plus") || model.contains("coder-plus") {
            return Some(QwenPlus);
        }
        if model.contains("flash") || model.contains("small") || model.contains("8b") {
            return Some(QwenFlash);
        }
        return Some(QwenDefault);
    }
    if model.contains("kimi") || model.contains("moonshot") {
        if model.contains("k2.7") || model.contains("k2.6") || model.contains("k2.5") {
            return Some(KimiLarge);
        }
        return Some(KimiDefault);
    }
    if model.contains("command") || model.contains("north") {
        if model.contains("r+") || model.contains("a") {
            return Some(CoherePremium);
        }
        return Some(CohereStandard);
    }
    if model.contains("nova") {
        if model.contains("premier") || model.contains("pro") {
            return Some(NovaPremier);
        }
        if model.contains("lite") {
            return Some(NovaLite);
        }
        return Some(NovaMicro);
    }
    if model.contains("sonar") {
        if model.contains("pro") || model.contains("deep") {
            return Some(SonarPro);
        }
        return Some(SonarBasic);
    }
    if model.contains("nemotron") {
        if model.contains("ultra") || model.contains("super") {
            return Some(NemotronLarge);
        }
        return Some(NemotronDefault);
    }
    if model.contains("compound") {
        return Some(GroqCompound);
    }
    if model.contains("llama-4") || model.contains("llama-3.3-70") || model.contains("llama3-70") {
        return Some(Llama70b);
    }
    if model.contains("llama-3.1-70")
        || model.contains("llama-3.1-8b")
        || model.contains("llama3-8b")
    {
        if model.contains("70") {
            return Some(Llama70b);
        }
        return Some(Llama8b);
    }
    if model.contains("llama-guard") || model.contains("prompt-guard") {
        return Some(LlamaGuard);
    }
    if model.contains("llama") {
        if model.contains("405") || model.contains("70") {
            return Some(Llama70b);
        }
        return Some(Llama8b);
    }
    if model.contains("phi-") {
        if model.contains("mini") {
            return Some(PhiMini);
        }
        return Some(PhiDefault);
    }
    if model.contains("wizardlm") || model.contains("hermes") || model.contains("olmo") {
        return Some(MidTier90);
    }
    if model.contains("granite") {
        return Some(MidTier10);
    }
    if model.contains("solar") || model.contains("hunyuan") || model.contains("ernie") {
        return Some(MidTier30);
    }
    if model.contains("seed-") || model.contains("mimo") || model.contains("step-") {
        return Some(MidTier60);
    }
    if model.contains("mercury") || model.contains("ling-") || model.contains("ring-") {
        return Some(MidTier80);
    }
    if model.contains("jamba") || model.contains("aion") {
        return Some(CohereStandard);
    }
    if model.contains("kat-coder") || model.contains("coder-large") || model.contains("virtuoso") {
        return Some(MidTier90);
    }
    if model.contains("trinity") {
        return Some(MidTier60);
    }
    if model.contains("magnum") || model.contains("mythomax") || model.contains("cydonia") {
        return Some(MidTier80);
    }
    if model.contains("lfm-") || model.contains("liquid") {
        return Some(Budget05);
    }
    if model.contains("weaver") || model.contains("morph") || model.contains("relace") {
        return Some(MidTier30);
    }
    if model.contains("reka") || model.contains("fugu") || model.contains("palmyra") {
        return Some(MidTier30);
    }
    if model.contains("inflection") || model.contains("perceptron") {
        return Some(CohereStandard);
    }
    if model.contains("poolside") || model.contains("laguna") {
        return Some(MidTier30);
    }
    if model.contains("cogito") {
        return Some(OpenAiCodexMini);
    }
    if model.contains("dolphin") || model.contains("venice") {
        return Some(MidTier10);
    }
    if model.contains("router") || model.contains("switchpoint") {
        return Some(CohereStandard);
    }
    if model.contains("nex-n") {
        return Some(NemotronLarge);
    }
    if model.contains("slerp")
        || model.contains("rocinante")
        || model.contains("skyfall")
        || model.contains("unslopnemo")
        || model.contains("slopnemo")
    {
        return Some(MidTier10);
    }
    if model.starts_with("l3")
        || model.contains("lunaris")
        || model.contains("euryale")
        || model.contains("hanami")
    {
        if model.contains("70") {
            return Some(Budget20);
        }
        return Some(Budget10);
    }
    if model.contains("hy3") || model.starts_with("hy") {
        return Some(MidTier30);
    }
    if model.contains("ui-tars") {
        return Some(Budget05);
    }
    None
}

/// Map canonical model id → price tier (pattern rules).
fn classify(model: &str) -> Option<PriceTier> {
    for (id, tier) in EXACT_TIERS {
        if model == *id {
            return Some(*tier);
        }
    }

    classify_anthropic(model)
        .or_else(|| classify_openai(model))
        .or_else(|| classify_google(model))
        .or_else(|| classify_deepseek(model))
        .or_else(|| classify_mistral(model))
        .or_else(|| classify_xai(model))
        .or_else(|| classify_zai(model))
        .or_else(|| classify_minimax(model))
        .or_else(|| classify_open_catalog(model))
}

/// Returns `(input_per_MTok, cache_write_per_MTok, cache_read_per_MTok, output_per_MTok)`.
pub fn native_model_pricing(model: &str) -> Option<PerMTok> {
    let model = canonical_model_name(model);
    classify(&model).map(tier_rates)
}

pub fn format_native_pricing_lines(model: &str) -> Vec<String> {
    match native_model_pricing(model) {
        Some((input, cache_write, cache_read, output)) => {
            let mut lines = vec![format!(
                "Pricing:   ${input:.2} in / ${output:.2} out (per MTok)"
            )];
            if cache_write > 0.0 || cache_read > 0.0 {
                lines.push(format!(
                    "           ${cache_write:.2} cache write / ${cache_read:.2} cache read (per MTok)"
                ));
            }
            lines
        }
        None => vec![String::from("Pricing:   unknown")],
    }
}
