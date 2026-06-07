use crate::services::custom_model_store;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Llm,
    Encoder,
    Nli,
}

impl ModelKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelKind::Llm => "llm",
            ModelKind::Encoder => "encoder",
            ModelKind::Nli => "nli",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "encoder" => ModelKind::Encoder,
            "nli" => ModelKind::Nli,
            _ => ModelKind::Llm,
        }
    }

    pub fn uses_embeddings(&self) -> bool {
        matches!(self, ModelKind::Encoder)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filename: String,
    pub download_url: String,
    pub size_hint_bytes: u64,
    pub expected_sha256: Option<String>,
    pub min_vram_gb: u8,
    pub layer_count_hint: u32,
    pub recommended: bool,
    pub deprecated_reason: Option<String>,
    pub is_custom: bool,
    pub model_kind: ModelKind,
}

struct BuiltinModelDefinition {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    filename: &'static str,
    download_url: &'static str,
    size_hint_bytes: u64,
    expected_sha256: Option<&'static str>,
    min_vram_gb: u8,
    layer_count_hint: u32,
    recommended: bool,
    deprecated_reason: Option<&'static str>,
    model_kind: ModelKind,
}

impl BuiltinModelDefinition {
    fn to_definition(&self) -> ModelDefinition {
        ModelDefinition {
            id: self.id.to_string(),
            name: self.name.to_string(),
            description: self.description.to_string(),
            filename: self.filename.to_string(),
            download_url: self.download_url.to_string(),
            size_hint_bytes: self.size_hint_bytes,
            expected_sha256: self.expected_sha256.map(String::from),
            min_vram_gb: self.min_vram_gb,
            layer_count_hint: self.layer_count_hint,
            recommended: self.recommended,
            deprecated_reason: self.deprecated_reason.map(String::from),
            is_custom: false,
            model_kind: self.model_kind.clone(),
        }
    }
}

fn llm_catalog() -> &'static [BuiltinModelDefinition] {
    &[
        BuiltinModelDefinition {
            id: "vikhr-nemo-12b-instruct",
            name: "Vikhr-Nemo 12B Instruct",
            description: "Vikhrmodels/Vikhr-Nemo-12B-Instruct · русская LLM для генерации и проверки дублей (~7 ГБ Q4, 12+ ГБ VRAM или гибрид)",
            filename: "Vikhr-Nemo-12B-Instruct-R-21-09-24-Q4_K_M.gguf",
            download_url: "https://huggingface.co/bartowski/Vikhr-Nemo-12B-Instruct-R-21-09-24-GGUF/resolve/main/Vikhr-Nemo-12B-Instruct-R-21-09-24-Q4_K_M.gguf",
            size_hint_bytes: 7_477_218_976,
            expected_sha256: None,
            min_vram_gb: 10,
            layer_count_hint: 40,
            recommended: true,
            deprecated_reason: None,
            model_kind: ModelKind::Llm,
        },
        BuiltinModelDefinition {
            id: "qwen2.5-14b-instruct",
            name: "Qwen2.5 14B Instruct",
            description: "Qwen/Qwen2.5-14B-Instruct · сильнее для перевода EN→RU и JSON (~9 ГБ Q4, 12+ ГБ VRAM или гибрид ~24 слоя на 8 ГБ)",
            filename: "Qwen2.5-14B-Instruct-Q4_K_M.gguf",
            download_url: "https://huggingface.co/bartowski/Qwen2.5-14B-Instruct-GGUF/resolve/main/Qwen2.5-14B-Instruct-Q4_K_M.gguf",
            size_hint_bytes: 9_148_278_443,
            expected_sha256: None,
            min_vram_gb: 12,
            layer_count_hint: 48,
            recommended: false,
            deprecated_reason: None,
            model_kind: ModelKind::Llm,
        },
        BuiltinModelDefinition {
            id: "llama-3.1-8b-instruct",
            name: "Llama 3.1 8B Instruct",
            description: "meta-llama/Llama-3.1-8B-Instruct",
            filename: "Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf",
            download_url: "https://huggingface.co/bartowski/Meta-Llama-3.1-8B-Instruct-GGUF/resolve/main/Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf",
            size_hint_bytes: 4_920_739_232,
            expected_sha256: Some(
                "7b064f5842bf9532c91456deda288a1b672397a54fa729aa665952863033557c",
            ),
            min_vram_gb: 8,
            layer_count_hint: 32,
            recommended: false,
            deprecated_reason: Some("Снята с рекомендаций: слабее для русских новостей"),
            model_kind: ModelKind::Llm,
        },
        BuiltinModelDefinition {
            id: "mistral-7b-v0.3",
            name: "Mistral 7B Instruct v0.3",
            description: "Mistral 7B Instruct v0.3",
            filename: "Mistral-7B-Instruct-v0.3-Q4_K_M.gguf",
            download_url: "https://huggingface.co/bartowski/Mistral-7B-Instruct-v0.3-GGUF/resolve/main/Mistral-7B-Instruct-v0.3-Q4_K_M.gguf",
            size_hint_bytes: 4_372_812_000,
            expected_sha256: None,
            min_vram_gb: 6,
            layer_count_hint: 32,
            recommended: false,
            deprecated_reason: Some("Снята с рекомендаций: слабее для русских новостей"),
            model_kind: ModelKind::Llm,
        },
        BuiltinModelDefinition {
            id: "deepseek-r1-7b",
            name: "DeepSeek R1 7B",
            description: "Reasoning-модель",
            filename: "deepseek-r1-7b-q4_k_m.gguf",
            download_url: "https://huggingface.co/bartowski/DeepSeek-R1-Distill-Qwen-7B-GGUF/resolve/main/DeepSeek-R1-Distill-Qwen-7B-Q4_K_M.gguf",
            size_hint_bytes: 4_683_073_504,
            expected_sha256: None,
            min_vram_gb: 6,
            layer_count_hint: 28,
            recommended: false,
            deprecated_reason: Some("Reasoning-модель: медленная, часто ломает JSON"),
            model_kind: ModelKind::Llm,
        },
    ]
}

fn builtin_catalog() -> impl Iterator<Item = &'static BuiltinModelDefinition> {
    llm_catalog().iter()
}

pub fn all_models() -> Vec<ModelDefinition> {
    let mut models: Vec<ModelDefinition> = builtin_catalog()
        .map(|m| m.to_definition())
        .collect();
    if let Ok(custom) = custom_model_store::load_all() {
        for record in custom {
            models.push(record.to_definition());
        }
    }
    models
}

pub fn catalog() -> Vec<ModelDefinition> {
    all_models()
}

pub fn find(id: &str) -> Option<ModelDefinition> {
    let normalized = normalize_model_id(id);
    builtin_catalog()
        .find(|m| m.id == normalized)
        .map(|m| m.to_definition())
        .or_else(|| {
            custom_model_store::load_all()
                .ok()?
                .into_iter()
                .find(|m| m.id == normalized)
                .map(|m| m.to_definition())
        })
}

pub fn find_by_filename(filename: &str) -> Option<ModelDefinition> {
    all_models()
        .into_iter()
        .find(|m| m.filename.eq_ignore_ascii_case(filename))
}

pub fn normalize_model_id(id: &str) -> &str {
    match id {
        "deepseek-r1-7b-q4" => "deepseek-r1-7b",
        other => other,
    }
}

pub fn llm_model_selectable(id: &str) -> bool {
    let id = normalize_model_id(id);
    find(id).is_some_and(|def| {
        def.model_kind == ModelKind::Llm && def.deprecated_reason.is_none()
    })
}

pub fn default_model_id() -> &'static str {
    "vikhr-nemo-12b-instruct"
}

pub fn default_dedup_model_id() -> &'static str {
    default_model_id()
}

pub fn resolve_ngl(device: &str, gpu_layers: u32) -> u32 {
    match device {
        "cpu" => 0,
        "hybrid" => gpu_layers.clamp(1, 99),
        _ => 99,
    }
}

pub fn resolve_ngl_for_model(device: &str, gpu_layers: u32, def: &ModelDefinition) -> u32 {
    if def.model_kind.uses_embeddings() {
        match device {
            "cpu" => 0,
            _ => def.layer_count_hint.min(99),
        }
    } else {
        resolve_ngl(device, gpu_layers)
    }
}
