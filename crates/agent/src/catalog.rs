//! Model catalog — normalized model records with capabilities.
//!
//! Each provider's raw model list is parsed into `ModelInfo`.  Providers
//! that return only ids are enriched from a static hints table; providers
//! that expose entitlements (subscription plans) filter their list here, so
//! the UI only ever sees models the authenticated account may actually use.

use serde_json::Value;

/// One selectable reasoning setting.  `param` carries the provider-specific
/// wire value (e.g. "high", a thinking budget in tokens, "enabled").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasoningLevel {
    pub id: String,
    pub label: String,
    pub param: String,
}

fn level(id: &str, label: &str, param: &str) -> ReasoningLevel {
    ReasoningLevel {
        id: id.into(),
        label: label.into(),
        param: param.into(),
    }
}

/// What reasoning control a model exposes, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReasoningSupport {
    None,
    /// A single on/off style choice set (e.g. enabled/disabled).
    Toggle(Vec<ReasoningLevel>),
    /// Ordered effort levels (minimal → extra high).
    Levels(Vec<ReasoningLevel>),
}

impl ReasoningSupport {
    pub fn levels(&self) -> &[ReasoningLevel] {
        match self {
            ReasoningSupport::None => &[],
            ReasoningSupport::Toggle(l) | ReasoningSupport::Levels(l) => l,
        }
    }

    pub fn default_level(&self) -> Option<&ReasoningLevel> {
        self.levels().first()
    }
}

/// One model the authenticated account can use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    pub display: String,
    pub context_window: Option<u64>,
    pub max_output: Option<u64>,
    pub reasoning: ReasoningSupport,
    pub tools: bool,
    pub image_input: bool,
    /// Provider note (e.g. "preview") shown beside the name.
    pub note: String,
}

impl ModelInfo {
    /// Toolbar-style label: display name plus context size when known.
    pub fn label(&self) -> String {
        match (self.context_window, self.note.as_str()) {
            (Some(ctx), "") => format!("{} · {}", self.display, format_tokens(ctx)),
            (Some(ctx), note) => format!("{} · {} ({})", self.display, format_tokens(ctx), note),
            (None, "") => self.display.clone(),
            (None, note) => format!("{} ({})", self.display, note),
        }
    }
}

/// "128000" → "128k", "1048576" → "1.0M".
pub fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 && n % 1_000_000 == 0 {
        format!("{}M", n / 1_000_000)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 && n % 1_000 == 0 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

/// The standard effort ladder shared by effort-style providers.
pub fn effort_levels(include_minimal: bool) -> Vec<ReasoningLevel> {
    let mut out = vec![];
    if include_minimal {
        out.push(level("minimal", "Minimal", "minimal"));
    }
    out.extend([
        level("low", "Low", "low"),
        level("medium", "Medium", "medium"),
        level("high", "High", "high"),
    ]);
    out
}

/// An on/off reasoning toggle for models with one.
pub fn toggle_levels(on: &str, off: &str) -> Vec<ReasoningLevel> {
    vec![level("on", "Enabled", on), level("off", "Disabled", off)]
}

/// Anthropic thinking budgets; `param` is the budget in tokens.
pub fn thinking_levels() -> Vec<ReasoningLevel> {
    vec![
        level("low", "Low", "2048"),
        level("medium", "Medium", "8192"),
        level("high", "High", "16384"),
    ]
}

// ── Capability hints for providers that list bare ids ────────────────────────

/// Const-friendly reasoning description; expanded into a `ReasoningSupport`
/// when a hint is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HintReasoning {
    None,
    Effort { minimal: bool },
    Toggle,
}

pub struct Hint {
    matches: &'static str,
    display: &'static str,
    context: u64,
    reasoning: HintReasoning,
    tools: bool,
    image: bool,
    note: &'static str,
}

const OPENAI_HINTS: &[Hint] = &[
    Hint { matches: "gpt-5.1", display: "GPT-5.1", context: 400_000, reasoning: HintReasoning::Effort { minimal: true }, tools: true, image: true, note: "" },
    Hint { matches: "gpt-5", display: "GPT-5", context: 400_000, reasoning: HintReasoning::Effort { minimal: true }, tools: true, image: true, note: "" },
    Hint { matches: "gpt-4.1", display: "GPT-4.1", context: 1_047_576, reasoning: HintReasoning::None, tools: true, image: true, note: "" },
    Hint { matches: "gpt-4o", display: "GPT-4o", context: 128_000, reasoning: HintReasoning::None, tools: true, image: true, note: "" },
    Hint { matches: "o4-mini", display: "o4-mini", context: 200_000, reasoning: HintReasoning::Effort { minimal: false }, tools: true, image: true, note: "" },
    Hint { matches: "o3", display: "o3", context: 200_000, reasoning: HintReasoning::Effort { minimal: false }, tools: true, image: true, note: "" },
    Hint { matches: "o1", display: "o1", context: 200_000, reasoning: HintReasoning::Effort { minimal: false }, tools: true, image: false, note: "" },
];

pub const GENERIC_HINTS: &[Hint] = &[
    // DeepSeek
    Hint { matches: "deepseek-reasoner", display: "DeepSeek Reasoner", context: 128_000, reasoning: HintReasoning::None, tools: true, image: false, note: "" },
    Hint { matches: "deepseek-chat", display: "DeepSeek Chat", context: 128_000, reasoning: HintReasoning::None, tools: true, image: false, note: "" },
    // Kimi / Moonshot
    Hint { matches: "kimi-k2", display: "Kimi K2", context: 256_000, reasoning: HintReasoning::None, tools: true, image: false, note: "" },
    Hint { matches: "moonshot", display: "Moonshot", context: 128_000, reasoning: HintReasoning::None, tools: true, image: false, note: "" },
    // Z.ai GLM
    Hint { matches: "glm", display: "GLM", context: 128_000, reasoning: HintReasoning::Toggle, tools: true, image: false, note: "" },
];

fn expand_reasoning(kind: HintReasoning) -> ReasoningSupport {
    match kind {
        HintReasoning::None => ReasoningSupport::None,
        HintReasoning::Effort { minimal } => ReasoningSupport::Levels(effort_levels(minimal)),
        HintReasoning::Toggle => ReasoningSupport::Toggle(toggle_levels("enabled", "disabled")),
    }
}

fn hint_for<'a>(hints: &'a [Hint], id: &str) -> Option<&'a Hint> {
    hints.iter().find(|h| id.contains(h.matches))
}

fn enrich(hints: &[Hint], id: &str) -> ModelInfo {
    match hint_for(hints, id) {
        Some(h) => ModelInfo {
            id: id.into(),
            display: h.display.into(),
            context_window: Some(h.context),
            max_output: None,
            reasoning: expand_reasoning(h.reasoning),
            tools: h.tools,
            image_input: h.image,
            note: h.note.into(),
        },
        None => ModelInfo {
            id: id.into(),
            display: id.into(),
            context_window: None,
            max_output: None,
            reasoning: ReasoningSupport::None,
            tools: true,
            image_input: false,
            note: String::new(),
        },
    }
}

// ── Parsers ──────────────────────────────────────────────────────────────────

fn json_or_err(raw: &str, what: &str) -> Result<Value, String> {
    serde_json::from_str(raw).map_err(|e| format!("{what}: bad JSON: {e}"))
}

fn api_error(v: &Value) -> Option<String> {
    v.get("error").map(|e| {
        e.get("message")
            .and_then(Value::as_str)
            .unwrap_or("provider rejected the request")
            .to_string()
    })
}

/// OpenAI-style `{data: [{id}]}` list, enriched from the hints table.
pub fn parse_openai_models(raw: &str) -> Result<Vec<ModelInfo>, String> {
    let v = json_or_err(raw, "models")?;
    if let Some(err) = api_error(&v) {
        return Err(err);
    }
    let mut out: Vec<ModelInfo> = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or("models response missing data array")?
        .iter()
        .filter_map(|m| m.get("id").and_then(Value::as_str))
        .map(|id| enrich(OPENAI_HINTS, id))
        .collect();
    out.sort_by(|a, b| a.display.to_lowercase().cmp(&b.display.to_lowercase()));
    Ok(out)
}

/// Anthropic `{data: [{id, display_name}]}`.  Claude 3.7+ and 4.x expose an
/// extended-thinking control; the catalog translates it into budgets.
pub fn parse_anthropic_models(raw: &str) -> Result<Vec<ModelInfo>, String> {
    let v = json_or_err(raw, "models")?;
    if let Some(err) = api_error(&v) {
        return Err(err);
    }
    let mut out: Vec<ModelInfo> = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or("models response missing data array")?
        .iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(Value::as_str)?;
            let display = m
                .get("display_name")
                .and_then(Value::as_str)
                .unwrap_or(id);
            let thinking = supports_thinking(id);
            let mut info = ModelInfo {
                id: id.into(),
                display: display.into(),
                context_window: Some(200_000),
                max_output: Some(if thinking { 65_536 } else { 8_192 }),
                reasoning: if thinking {
                    ReasoningSupport::Levels(thinking_levels())
                } else {
                    ReasoningSupport::None
                },
                tools: true,
                image_input: true,
                note: String::new(),
            };
            if let Some(ctx) = context_hint_for_claude(id) {
                info.context_window = Some(ctx);
            }
            Some(info)
        })
        .collect();
    out.sort_by(|a, b| a.display.to_lowercase().cmp(&b.display.to_lowercase()));
    Ok(out)
}

fn supports_thinking(id: &str) -> bool {
    id.starts_with("claude-")
        && !id.starts_with("claude-3-5")
        && !id.starts_with("claude-3-opus")
        && !id.starts_with("claude-3-haiku")
}

fn context_hint_for_claude(id: &str) -> Option<u64> {
    if id.contains("haiku") {
        Some(200_000)
    } else {
        None
    }
}

/// OpenRouter `{data: [{id, name, context_length, architecture,
/// supported_parameters}]}` — the richest source: capabilities come with the
/// list, and every listed model is usable by the key.
pub fn parse_openrouter_models(raw: &str) -> Result<Vec<ModelInfo>, String> {
    let v = json_or_err(raw, "models")?;
    if let Some(err) = api_error(&v) {
        return Err(err);
    }
    let mut out: Vec<ModelInfo> = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or("models response missing data array")?
        .iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(Value::as_str)?;
            let display = m
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(id)
                .split(':')
                .last()
                .unwrap_or(id)
                .trim()
                .to_string();
            let params: Vec<&str> = m
                .get("supported_parameters")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let reasoning = if params.contains(&"reasoning_effort") || params.contains(&"reasoning")
            {
                ReasoningSupport::Levels(effort_levels(true))
            } else if params.contains(&"include_reasoning") || params.contains(&"thinking") {
                ReasoningSupport::Toggle(toggle_levels("enabled", "disabled"))
            } else {
                ReasoningSupport::None
            };
            let image = m
                .pointer("/architecture/input_modalities")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).any(|s| s == "image"))
                .unwrap_or(false);
            Some(ModelInfo {
                id: id.into(),
                display,
                context_window: m.get("context_length").and_then(Value::as_u64),
                max_output: m
                    .pointer("/top_provider/max_completion_tokens")
                    .and_then(Value::as_u64),
                reasoning,
                tools: params.contains(&"tools"),
                image_input: image,
                note: m
                    .pointer("/pricing/prompt")
                    .and_then(Value::as_str)
                    .filter(|p| *p == "0")
                    .map(|_| "free".into())
                    .unwrap_or_default(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.display.to_lowercase().cmp(&b.display.to_lowercase()));
    Ok(out)
}

/// Copilot `{data: [{id, name, capabilities, policy}]}` — the subscription
/// model: the list is already entitlement-filtered server-side, and
/// `policy.state` / `model_picker_enabled` gate individual entries.
pub fn parse_copilot_models(raw: &str) -> Result<Vec<ModelInfo>, String> {
    let v = json_or_err(raw, "models")?;
    if let Some(err) = api_error(&v) {
        return Err(err);
    }
    let mut out: Vec<ModelInfo> = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or("models response missing data array")?
        .iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(Value::as_str)?;
            let kind = m.pointer("/capabilities/type").and_then(Value::as_str)?;
            if kind != "chat" {
                return None;
            }
            // Entitlement gates: the subscription decides what shows up.
            let state = m.pointer("/policy/state").and_then(Value::as_str);
            if state == Some("disabled") || state == Some("unavailable") {
                return None;
            }
            if m.get("model_picker_enabled") == Some(&Value::Bool(false)) {
                return None;
            }
            let caps = m.get("capabilities");
            let supports = caps.and_then(|c| c.get("supports"));
            let reasoning = if supports
                .and_then(|s| s.get("reasoning_effort"))
                .is_some_and(|r| r != &Value::Bool(false))
            {
                ReasoningSupport::Levels(effort_levels(true))
            } else {
                ReasoningSupport::None
            };
            Some(ModelInfo {
                id: id.into(),
                display: m
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_string(),
                context_window: caps
                    .and_then(|c| c.get("max_input_tokens"))
                    .and_then(Value::as_u64),
                max_output: caps
                    .and_then(|c| c.get("max_output_tokens"))
                    .and_then(Value::as_u64),
                reasoning,
                tools: supports
                    .and_then(|s| s.get("tool_calls"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                image_input: supports
                    .and_then(|s| s.get("vision"))
                    .and_then(Value::as_bool)
                    .or_else(|| {
                        supports
                            .and_then(|s| s.get("image_inputs"))
                            .and_then(Value::as_bool)
                    })
                    .unwrap_or(false),
                note: String::new(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.display.to_lowercase().cmp(&b.display.to_lowercase()));
    Ok(out)
}

/// Fallback catalog for API-key providers whose `/models` endpoint is
/// unavailable: a small known list, enriched from the generic hints.
pub fn fallback_models(hints: &[Hint], ids: &[&str]) -> Vec<ModelInfo> {
    ids.iter().map(|id| enrich(hints, id)).collect()
}

pub fn generic_hints() -> &'static [Hint] {
    GENERIC_HINTS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_models_enrich_bare_ids() {
        let raw = r#"{"data":[{"id":"gpt-4o"},{"id":"gpt-5"},{"id":"o3"},{"id":"whisper-1"}]}"#;
        let models = parse_openai_models(raw).unwrap();
        assert_eq!(models.len(), 4);
        let gpt5 = models.iter().find(|m| m.id == "gpt-5").unwrap();
        assert_eq!(gpt5.display, "GPT-5");
        assert_eq!(gpt5.context_window, Some(400_000));
        let levels = gpt5.reasoning.levels();
        assert_eq!(levels[0].id, "minimal");
        assert_eq!(levels.iter().last().unwrap().label, "High");
        let o3 = models.iter().find(|m| m.id == "o3").unwrap();
        assert_eq!(o3.reasoning.levels()[0].id, "low", "o-series has no minimal");
        let whisper = models.iter().find(|m| m.id == "whisper-1").unwrap();
        assert_eq!(whisper.display, "whisper-1", "unknown ids degrade to themselves");
        assert!(whisper.reasoning.levels().is_empty());
    }

    #[test]
    fn openai_models_surface_api_errors() {
        let err = parse_openai_models(r#"{"error":{"message":"Invalid API key"}}"#).unwrap_err();
        assert!(err.contains("Invalid API key"));
    }

    #[test]
    fn anthropic_models_expose_thinking_budgets() {
        let raw = r#"{"data":[
            {"id":"claude-sonnet-4-5","display_name":"Claude Sonnet 4.5"},
            {"id":"claude-3-5-haiku-latest","display_name":"Claude 3.5 Haiku"}
        ]}"#;
        let models = parse_anthropic_models(raw).unwrap();
        let sonnet = models
            .iter()
            .find(|m| m.id == "claude-sonnet-4-5")
            .expect("sonnet listed");
        assert_eq!(sonnet.display, "Claude Sonnet 4.5");
        let budgets = sonnet.reasoning.levels();
        assert_eq!(budgets.len(), 3);
        assert_eq!(budgets[0].param, "2048", "budget travels in param");
        assert!(sonnet.max_output.unwrap_or(0) > budgets[2].param.parse::<u64>().unwrap(),
            "max output must exceed the largest thinking budget");
        let haiku35 = models
            .iter()
            .find(|m| m.id == "claude-3-5-haiku-latest")
            .expect("haiku listed");
        assert_eq!(haiku35.reasoning, ReasoningSupport::None);
    }

    #[test]
    fn openrouter_models_read_capabilities_from_the_list() {
        let raw = r#"{"data":[
            {"id":"z-ai/glm-4.6","name":"Z.AI: GLM 4.6","context_length":200000,
             "architecture":{"input_modalities":["text","image"]},
             "supported_parameters":["tools","reasoning_effort"],
             "top_provider":{"max_completion_tokens":96000},
             "pricing":{"prompt":"0.4"}},
            {"id":"deepseek/deepseek-chat","name":"DeepSeek: V3","context_length":64000,
             "architecture":{"input_modalities":["text"]},
             "supported_parameters":["tools"],"pricing":{"prompt":"0"}}
        ]}"#;
        let models = parse_openrouter_models(raw).unwrap();
        let glm = models.iter().find(|m| m.id.ends_with("glm-4.6")).unwrap();
        assert_eq!(glm.display, "GLM 4.6");
        assert!(glm.image_input);
        assert!(glm.tools);
        assert_eq!(glm.reasoning.levels()[0].id, "minimal");
        assert_eq!(glm.context_window, Some(200_000));
        assert_eq!(glm.max_output, Some(96_000));
        let deepseek = models.iter().find(|m| m.id.ends_with("deepseek-chat")).unwrap();
        assert!(!deepseek.image_input);
        assert_eq!(deepseek.note, "free");
        assert!(deepseek.reasoning.levels().is_empty());
    }

    #[test]
    fn copilot_models_filter_by_subscription_entitlements() {
        let raw = r#"{"data":[
            {"id":"gpt-5","name":"GPT-5","model_picker_enabled":true,
             "capabilities":{"type":"chat","supports":{"tool_calls":true,"reasoning_effort":{}},"max_input_tokens":400000},
             "policy":{"state":"active"}},
            {"id":"o1-preview","name":"o1-preview","model_picker_enabled":true,
             "capabilities":{"type":"chat","supports":{}},"policy":{"state":"disabled"}},
            {"id":"claude-sonnet-4","name":"Claude Sonnet 4","model_picker_enabled":false,
             "capabilities":{"type":"chat","supports":{}},"policy":{"state":"active"}},
            {"id":"text-embedding-3","name":"Embeddings","model_picker_enabled":true,
             "capabilities":{"type":"embedding","supports":{}},"policy":{"state":"active"}}
        ]}"#;
        let models = parse_copilot_models(raw).unwrap();
        assert_eq!(models.len(), 1, "entitlements and type gate the list: {models:?}");
        let gpt5 = &models[0];
        assert_eq!(gpt5.display, "GPT-5");
        assert!(gpt5.tools);
        assert!(gpt5.reasoning.levels().is_empty() || gpt5.reasoning.levels()[0].id == "minimal");
    }

    #[test]
    fn labels_carry_context_and_notes() {
        let mut m = enrich(OPENAI_HINTS, "gpt-4o");
        assert_eq!(m.label(), "GPT-4o · 128k");
        m.note = "preview".into();
        assert_eq!(m.label(), "GPT-4o · 128k (preview)");
        m.context_window = None;
        assert_eq!(m.label(), "GPT-4o (preview)");
        m.note = String::new();
        assert_eq!(m.label(), "GPT-4o");
    }

    #[test]
    fn token_formatting() {
        assert_eq!(format_tokens(128_000), "128k");
        assert_eq!(format_tokens(1_048_576), "1.0M");
        assert_eq!(format_tokens(2_000_000), "2M");
        assert_eq!(format_tokens(999), "999");
    }

    #[test]
    fn fallback_catalog_enriches_known_ids() {
        let models = fallback_models(GENERIC_HINTS, &["deepseek-chat"]);
        assert_eq!(models[0].display, "DeepSeek Chat");
    }
}
