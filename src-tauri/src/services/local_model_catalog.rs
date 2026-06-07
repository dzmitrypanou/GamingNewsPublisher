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
        matches!(self, ModelKind::Encoder | ModelKind::Nli)
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

fn encoder_catalog() -> &'static [BuiltinModelDefinition] {
    &[
        BuiltinModelDefinition {
            id: "rubert-tiny2",
            name: "RuBERT Tiny2",
            description: "cointegrated/rubert-tiny2 · компактный русский энкодер (~110 МБ)",
            filename: "rubert-mini-uncased-q8_0.gguf",
            download_url: "https://huggingface.co/sergeyzh/rubert-mini-uncased-GGUF/resolve/main/rubert-mini-uncased-q8_0.gguf",
            size_hint_bytes: 41_940_448,
            expected_sha256: None,
            min_vram_gb: 2,
            layer_count_hint: 12,
            recommended: true,
            deprecated_reason: None,
            model_kind: ModelKind::Encoder,
        },
        BuiltinModelDefinition {
            id: "stella-en-ru-v1",
            name: "Stella EN-RU v1",
            description: "dunzhang/stella_en_ru_v1 · энкодер EN/RU для семантического поиска",
            filename: "stella_en_1.5B_v5.gguf",
            download_url: "https://huggingface.co/abhishekbhakat/stella_en_1.5B_v5_GGUF/resolve/main/stella_en_1.5B_v5.gguf",
            size_hint_bytes: 3_092_769_024,
            expected_sha256: None,
            min_vram_gb: 6,
            layer_count_hint: 28,
            recommended: false,
            deprecated_reason: None,
            model_kind: ModelKind::Encoder,
        },
        BuiltinModelDefinition {
            id: "bge-m3",
            name: "BGE-M3",
            description: "BAAI/bge-m3 · мультиязычный энкодер, dense+sparse (~2.1 ГБ FP16, Q4 ~440 МБ)",
            filename: "bge-m3-q4_k_m.gguf",
            download_url: "https://huggingface.co/bbvch-ai/bge-m3-GGUF/resolve/main/bge-m3-q4_k_m.gguf",
            size_hint_bytes: 437_778_496,
            expected_sha256: None,
            min_vram_gb: 4,
            layer_count_hint: 24,
            recommended: true,
            deprecated_reason: None,
            model_kind: ModelKind::Encoder,
        },
        BuiltinModelDefinition {
            id: "multilingual-e5-large-instruct",
            name: "E5 Large Instruct",
            description: "intfloat/multilingual-e5-large-instruct · мультиязычный энкодер (~2.2 ГБ FP16, Q6 ~470 МБ)",
            filename: "multilingual-e5-large-instruct-q6_k.gguf",
            download_url: "https://huggingface.co/Ralriki/multilingual-e5-large-instruct-GGUF/resolve/main/multilingual-e5-large-instruct-q6_k.gguf",
            size_hint_bytes: 467_958_912,
            expected_sha256: None,
            min_vram_gb: 4,
            layer_count_hint: 24,
            recommended: false,
            deprecated_reason: None,
            model_kind: ModelKind::Encoder,
        },
        BuiltinModelDefinition {
            id: "symanto-sn-xlm-roberta-nli",
            name: "XLM-RoBERTa NLI",
            description: "symanto/sn-xlm-roberta-base-snli-mnli-anli-xnli · NLI-классификатор (~1.1 ГБ FP16, F16 ~540 МБ)",
            filename: "XLM-Roberta.f16.gguf",
            download_url: "https://huggingface.co/mradermacher/XLM-Roberta-GGUF/resolve/main/XLM-Roberta.f16.gguf",
            size_hint_bytes: 563_953_248,
            expected_sha256: None,
            min_vram_gb: 4,
            layer_count_hint: 12,
            recommended: false,
            deprecated_reason: None,
            model_kind: ModelKind::Nli,
        },
    ]
}

fn llm_catalog() -> &'static [BuiltinModelDefinition] {
    &[
        BuiltinModelDefinition {
            id: "qwen2.5-7b-instruct",
            name: "Qwen2.5 7B Instruct",
            description: "Qwen/Qwen2.5-7B-Instruct · лучший выбор: русский, JSON, переписывание постов",
            filename: "Qwen2.5-7B-Instruct-Q4_K_M.gguf",
            download_url: "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf",
            size_hint_bytes: 4_683_074_240,
            expected_sha256: None,
            min_vram_gb: 6,
            layer_count_hint: 28,
            recommended: true,
            deprecated_reason: None,
            model_kind: ModelKind::Llm,
        },
        BuiltinModelDefinition {
            id: "vikhr-7b-instruct",
            name: "Vikhr 7B Instruct",
            description: "Русскоязычная instruct-модель для новостей",
            filename: "Vikhr-7B-instruct-Q4_K_M.gguf",
            download_url: "https://huggingface.co/oblivious/Vikhr-7B-instruct-GGUF/resolve/main/Vikhr-7B-instruct-Q4_K_M.gguf",
            size_hint_bytes: 4_413_985_472,
            expected_sha256: None,
            min_vram_gb: 6,
            layer_count_hint: 28,
            recommended: true,
            deprecated_reason: None,
            model_kind: ModelKind::Llm,
        },
        BuiltinModelDefinition {
            id: "vikhr-nemo-12b-instruct",
            name: "Vikhr-Nemo 12B Instruct",
            description: "Vikhrmodels/Vikhr-Nemo-12B-Instruct · сильная русская LLM (~8 ГБ Q4)",
            filename: "Vikhr-Nemo-12B-Instruct-R-21-09-24-Q4_K_M.gguf",
            download_url: "https://huggingface.co/bartowski/Vikhr-Nemo-12B-Instruct-R-21-09-24-GGUF/resolve/main/Vikhr-Nemo-12B-Instruct-R-21-09-24-Q4_K_M.gguf",
            size_hint_bytes: 7_477_218_976,
            expected_sha256: None,
            min_vram_gb: 10,
            layer_count_hint: 40,
            recommended: false,
            deprecated_reason: None,
            model_kind: ModelKind::Llm,
        },
        BuiltinModelDefinition {
            id: "llama-3.1-8b-instruct",
            name: "Llama 3.1 8B Instruct",
            description: "meta-llama/Llama-3.1-8B-Instruct · сильное следование промпту (~5.5 ГБ Q4)",
            filename: "Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf",
            download_url: "https://huggingface.co/bartowski/Meta-Llama-3.1-8B-Instruct-GGUF/resolve/main/Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf",
            size_hint_bytes: 4_920_739_232,
            expected_sha256: Some(
                "7b064f5842bf9532c91456deda288a1b672397a54fa729aa665952863033557c",
            ),
            min_vram_gb: 8,
            layer_count_hint: 32,
            recommended: false,
            deprecated_reason: None,
            model_kind: ModelKind::Llm,
        },
        BuiltinModelDefinition {
            id: "mistral-7b-v0.3",
            name: "Mistral 7B Instruct v0.3",
            description: "Быстрая и лёгкая instruct-модель",
            filename: "Mistral-7B-Instruct-v0.3-Q4_K_M.gguf",
            download_url: "https://huggingface.co/bartowski/Mistral-7B-Instruct-v0.3-GGUF/resolve/main/Mistral-7B-Instruct-v0.3-Q4_K_M.gguf",
            size_hint_bytes: 4_372_812_000,
            expected_sha256: None,
            min_vram_gb: 6,
            layer_count_hint: 32,
            recommended: false,
            deprecated_reason: None,
            model_kind: ModelKind::Llm,
        },
        BuiltinModelDefinition {
            id: "deepseek-r1-7b",
            name: "DeepSeek R1 7B",
            description: "Reasoning-модель; хуже для JSON и коротких постов",
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
    encoder_catalog()
        .iter()
        .chain(llm_catalog().iter())
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

pub fn default_model_id() -> &'static str {
    "qwen2.5-7b-instruct"
}

pub fn default_dedup_model_id() -> &'static str {
    "bge-m3"
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
