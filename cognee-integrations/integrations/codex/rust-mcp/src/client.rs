use crate::settings::Settings;
use anyhow::{Result, anyhow};
use reqwest::blocking::multipart::Form;
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::Value;

#[derive(Clone)]
pub(crate) struct CogneeClient {
    settings: Settings,
    client: Client,
}

impl CogneeClient {
    pub(crate) fn new(settings: Settings) -> Result<Self> {
        let client = Client::builder().timeout(settings.timeout).build()?;
        Ok(Self { settings, client })
    }

    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }

    pub(crate) fn get_json(&self, path: &str) -> Result<Value> {
        self.json_response(self.authorized(self.client.get(self.url(path))))
    }

    pub(crate) fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
        self.json_response(self.authorized(self.client.post(self.url(path))).json(body))
    }

    pub(crate) fn post_multipart(&self, path: &str, form: Form) -> Result<Value> {
        self.json_response(
            self.authorized(self.client.post(self.url(path)))
                .multipart(form),
        )
    }

    pub(crate) fn get_text(&self, path: &str) -> Result<String> {
        self.text_response(self.authorized(self.client.get(self.url(path))))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.settings.service_url, path)
    }

    fn authorized(&self, request: RequestBuilder) -> RequestBuilder {
        let request = self.with_primary_auth(request);
        self.with_tenant_header(request)
    }

    fn with_primary_auth(&self, request: RequestBuilder) -> RequestBuilder {
        if let Some(api_key) = &self.settings.api_key {
            request.header("X-Api-Key", api_key)
        } else if let Some(token) = &self.settings.bearer_token {
            request.bearer_auth(token)
        } else {
            request
        }
    }

    fn with_tenant_header(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.settings.tenant_id {
            Some(tenant_id) => request.header("X-Tenant-Id", tenant_id),
            None => request,
        }
    }

    fn json_response(&self, request: RequestBuilder) -> Result<Value> {
        let response = request.send()?;
        let status = response.status();
        let text = response.text()?;
        json_from_response(status.as_u16(), &text)
    }

    fn text_response(&self, request: RequestBuilder) -> Result<String> {
        let response = request.send()?;
        let status = response.status();
        let text = response.text()?;
        text_from_response(status.as_u16(), text)
    }
}

fn json_from_response(status: u16, text: &str) -> Result<Value> {
    if (200..300).contains(&status) {
        return Ok(serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string())));
    }
    Err(anyhow!("HTTP_{status}: {}", text))
}

fn text_from_response(status: u16, text: String) -> Result<String> {
    if (200..300).contains(&status) {
        return Ok(text);
    }
    Err(anyhow!("HTTP_{status}: {text}"))
}
