use anyhow::{Result, anyhow};
use reqwest::blocking::multipart::{Form, Part};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

pub(crate) fn form_with_uploads(values: &[String], max_bytes: u64) -> Result<Form> {
    values.iter().try_fold(Form::new(), |form, value| {
        append_upload(form, value, max_bytes)
    })
}

pub(crate) fn append_text_field(form: Form, name: &str, value: Option<String>) -> Form {
    match value {
        Some(text) => form.text(name.to_string(), text),
        None => form,
    }
}

pub(crate) fn append_bool_field(form: Form, name: &str, value: bool) -> Form {
    form.text(name.to_string(), value.to_string())
}

pub(crate) fn append_string_list_field(form: Form, name: &str, values: &[String]) -> Form {
    values.iter().fold(form, |form, value| {
        form.text(name.to_string(), value.clone())
    })
}

fn append_upload(form: Form, value: &str, max_bytes: u64) -> Result<Form> {
    let part = upload_part(value, max_bytes)?;
    Ok(form.part("data", part))
}

fn upload_part(value: &str, max_bytes: u64) -> Result<Part> {
    if Path::new(value).exists() {
        return file_part(value, max_bytes);
    }
    text_part(value, max_bytes)
}

fn file_part(path: &str, max_bytes: u64) -> Result<Part> {
    let bytes = fs::read(path)?;
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("upload.txt");
    bytes_part(bytes, name, max_bytes)
}

fn text_part(text: &str, max_bytes: u64) -> Result<Part> {
    let name = format!("text_{:x}.txt", text_hash(text));
    bytes_part(text.as_bytes().to_vec(), &name, max_bytes)
}

fn bytes_part(bytes: Vec<u8>, name: &str, max_bytes: u64) -> Result<Part> {
    if bytes.len() as u64 > max_bytes {
        return Err(anyhow!("upload exceeds COGNEE_MCP_MAX_UPLOAD_BYTES"));
    }
    Ok(Part::bytes(bytes).file_name(name.to_string()))
}

fn text_hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
