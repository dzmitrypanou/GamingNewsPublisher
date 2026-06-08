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
    pub embed_pooling: Option<String>,
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
    embed_pooling: Option<&'static str>,
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
            embed_pooling: self.embed_pooling.map(String::from),
        }
    }
}

fn llm_catalog() -> &'static [BuiltinModelDefinition] {
    &[
        BuiltinModelDefinition {
            id: "vikhr-nemo-12b-instruct",
            name: "Vikhr-Nemo 12B Instruct",
            description: "Vikhrmodels/Vikhr-Nemo-12B-Instruct · русская LLM для генерации (~7 ГБ Q4, 12+ ГБ VRAM или гибрид)",
            filename: "Vikhr-Nemo-12B-Instruct-R-21-09-24-Q4_K_M.gguf",
            download_url: "https://huggingface.co/bartowski/Vikhr-Nemo-12B-Instruct-R-21-09-24-GGUF/resolve/main/Vikhr-Nemo-12B-Instruct-R-21-09-24-Q4_K_M.gguf",
            size_hint_bytes: 7_477_218_976,
            expected_sha256: None,
            min_vram_gb: 10,
            layer_count_hint: 40,
            recommended: true,
            deprecated_reason: None,
            model_kind: ModelKind::Llm,
            embed_pooling: None,
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
            embed_pooling: None,
        },
    ]
}

fn encoder_catalog() -> &'static [BuiltinModelDefinition] {
    &[
        BuiltinModelDefinition {
            id: "multilingual-e5-base",
            name: "Multilingual E5 Base",
            description: "intfloat/multilingual-e5-base · быстрый энкодер для дублей, 100+ языков (~220 МБ Q4)",
            filename: "multilingual-e5-base-q4_k_m.gguf",
            download_url: "https://huggingface.co/jeremyys/multilingual-e5-base-Q4_K_M-GGUF/resolve/main/multilingual-e5-base-q4_k_m.gguf",
            size_hint_bytes: 219_152_384,
            expected_sha256: None,
            min_vram_gb: 2,
            layer_count_hint: 12,
            recommended: true,
            deprecated_reason: None,
            model_kind: ModelKind::Encoder,
            embed_pooling: Some("mean"),
        },
        BuiltinModelDefinition {
            id: "multilingual-e5-large",
            name: "Multilingual E5 Large",
            description: "intfloat/multilingual-e5-large · точнее E5 Base, те же префиксы query/passage (~400 МБ Q4)",
            filename: "multilingual-e5-large-q4_k_m.gguf",
            download_url: "https://huggingface.co/groonga/multilingual-e5-large-Q4_K_M-GGUF/resolve/main/multilingual-e5-large-q4_k_m.gguf",
            size_hint_bytes: 406_322_336,
            expected_sha256: None,
            min_vram_gb: 3,
            layer_count_hint: 24,
            recommended: false,
            deprecated_reason: None,
            model_kind: ModelKind::Encoder,
            embed_pooling: Some("mean"),
        },
        BuiltinModelDefinition {
            id: "bge-m3",
            name: "BGE-M3",
            description: "BAAI/bge-m3 · сильнее E5 на семантике, 100+ языков, 1024 dim (~440 МБ Q4)",
            filename: "bge-m3-q4_k_m.gguf",
            download_url: "https://huggingface.co/jeremyys/bge-m3-Q4_K_M-GGUF/resolve/main/bge-m3-q4_k_m.gguf",
            size_hint_bytes: 437_778_464,
            expected_sha256: None,
            min_vram_gb: 4,
            layer_count_hint: 24,
            recommended: false,
            deprecated_reason: None,
            model_kind: ModelKind::Encoder,
            embed_pooling: Some("mean"),
        },
    ]
}

fn all_builtin_catalog() -> impl Iterator<Item = &'static BuiltinModelDefinition> {
    llm_catalog().iter().chain(encoder_catalog().iter())
}

pub fn all_models() -> Vec<ModelDefinition> {
    let mut models: Vec<ModelDefinition> = all_builtin_catalog()
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
    all_builtin_catalog()
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
    id
}

pub fn is_llm_model(id: &str) -> bool {
    find(id).is_some_and(|def| def.model_kind == ModelKind::Llm)
}

pub fn is_encoder_model(id: &str) -> bool {
    find(id).is_some_and(|def| def.model_kind == ModelKind::Encoder)
}

pub fn generation_model_selectable(id: &str) -> bool {
    let id = normalize_model_id(id);
    find(id).is_some_and(|def| {
        def.model_kind == ModelKind::Llm && def.deprecated_reason.is_none()
    })
}

pub fn dedup_model_selectable(id: &str) -> bool {
    let id = normalize_model_id(id);
    find(id).is_some_and(|def| {
        matches!(def.model_kind, ModelKind::Llm | ModelKind::Encoder)
            && def.deprecated_reason.is_none()
    })
}

pub fn llm_model_selectable(id: &str) -> bool {
    generation_model_selectable(id)
}

pub fn default_model_id() -> &'static str {
    "vikhr-nemo-12b-instruct"
}

pub fn default_dedup_model_id() -> &'static str {
    "multilingual-e5-base"
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
