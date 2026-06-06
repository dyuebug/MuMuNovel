#[derive(Clone, Debug)]
pub struct AIConfig {
    pub provider: String,
    pub api_key: String,
    pub base_url: String,
    pub backup_urls: Vec<String>,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u32,
    pub system_prompt: Option<String>,
    pub max_retries: u32,
    pub request_delay_ms: u64,
    pub prefer_normalized_v1_candidate: bool,
    pub read_timeout_secs: Option<f64>,
    pub transport_max_retries: Option<u32>,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            api_key: String::new(),
            base_url: "https://api.openai.com/v1".into(),
            backup_urls: Vec::new(),
            model: "gpt-4".into(),
            temperature: 0.7,
            max_tokens: 32000,
            system_prompt: None,
            max_retries: 3,
            request_delay_ms: 200,
            prefer_normalized_v1_candidate: false,
            read_timeout_secs: None,
            transport_max_retries: None,
        }
    }
}
