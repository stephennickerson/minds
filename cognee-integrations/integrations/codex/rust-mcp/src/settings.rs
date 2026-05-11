use std::env;
use std::time::Duration;

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub(crate) service_url: String,
    pub(crate) api_key: Option<String>,
    pub(crate) bearer_token: Option<String>,
    pub(crate) tenant_id: Option<String>,
    pub(crate) timeout: Duration,
    pub(crate) top_k: u64,
    pub(crate) max_upload_bytes: u64,
    pub(crate) operator_tools_enabled: bool,
    pub(crate) destructive_tools_enabled: bool,
}

impl Settings {
    pub(crate) fn from_environment(service_url: Option<String>) -> Self {
        Self {
            service_url: configured_service_url(service_url),
            api_key: env_value("COGNEE_API_KEY"),
            bearer_token: env_value("COGNEE_BEARER_TOKEN"),
            tenant_id: env_value("COGNEE_TENANT_ID"),
            timeout: Duration::from_millis(number_value("COGNEE_TIMEOUT_MS", 30_000)),
            top_k: number_value("COGNEE_TOP_K", 10).clamp(1, 50),
            max_upload_bytes: number_value("COGNEE_MCP_MAX_UPLOAD_BYTES", 20_000_000),
            operator_tools_enabled: bool_value("COGNEE_MCP_ENABLE_OPERATOR_TOOLS"),
            destructive_tools_enabled: bool_value("COGNEE_MCP_ENABLE_DESTRUCTIVE_TOOLS"),
        }
    }

    pub(crate) fn auth_mode(&self) -> &'static str {
        if self.api_key.is_some() {
            "api-key"
        } else if self.bearer_token.is_some() {
            "bearer-token"
        } else {
            "unauthenticated"
        }
    }
}

fn configured_service_url(service_url: Option<String>) -> String {
    service_url
        .or_else(|| env_value("COGNEE_SERVICE_URL"))
        .unwrap_or_else(|| "http://localhost:8000".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn number_value(name: &str, fallback: u64) -> u64 {
    env_value(name)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(fallback)
}

fn bool_value(name: &str) -> bool {
    env_value(name)
        .map(|value| value.eq_ignore_ascii_case("true") || value == "1")
        .unwrap_or(false)
}
