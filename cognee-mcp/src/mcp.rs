use crate::client::CogneeClient;
use crate::tools;
use anyhow::Result;
use serde_json::{Value, json};
use std::env;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Read, Write};

const JSONRPC_VERSION: &str = "2.0";
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

#[derive(Clone, Copy)]
enum MessageMode {
    Framed,
    Line,
}

pub(crate) fn run_mcp(client: CogneeClient) -> Result<()> {
    debug_log("start");
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout();

    while let Some((message, mode)) = read_message(&mut reader)? {
        debug_log(&format!(
            "read {} {} {}",
            mode_name(mode),
            message.len(),
            debug_excerpt(&message)
        ));
        let Some(response) = response_for_line(&client, &message) else {
            debug_log("skip response");
            continue;
        };
        debug_log(&format!("write {}", debug_excerpt(&response.to_string())));
        write_message(&mut stdout, &response, mode)?;
    }

    debug_log("eof");
    Ok(())
}

fn read_message(reader: &mut BufReader<impl Read>) -> Result<Option<(String, MessageMode)>> {
    let mut first_line = String::new();
    if reader.read_line(&mut first_line)? == 0 {
        return Ok(None);
    }
    if first_line.trim().is_empty() {
        return read_message(reader);
    }
    if first_line.starts_with("Content-Length:") {
        return framed_message(reader, &first_line)
            .map(|message| Some((message, MessageMode::Framed)));
    }
    Ok(Some((first_line, MessageMode::Line)))
}

fn framed_message(reader: &mut BufReader<impl Read>, first_line: &str) -> Result<String> {
    let length = content_length(first_line)?;
    read_headers(reader)?;
    read_body(reader, length)
}

fn content_length(line: &str) -> Result<usize> {
    let Some((_, value)) = line.split_once(':') else {
        return Err(anyhow::anyhow!("missing Content-Length value"));
    };
    Ok(value.trim().parse()?)
}

fn read_headers(reader: &mut BufReader<impl Read>) -> Result<()> {
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.trim().is_empty() {
            return Ok(());
        }
    }
}

fn read_body(reader: &mut BufReader<impl Read>, length: usize) -> Result<String> {
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(String::from_utf8(body)?)
}

fn write_message(stdout: &mut impl Write, response: &Value, mode: MessageMode) -> Result<()> {
    let body = serde_json::to_vec(response)?;
    if matches!(mode, MessageMode::Framed) {
        write!(stdout, "Content-Length: {}\r\n\r\n", body.len())?;
    }
    stdout.write_all(&body)?;
    if matches!(mode, MessageMode::Line) {
        writeln!(stdout)?;
    }
    stdout.flush()?;
    Ok(())
}

fn response_for_line(client: &CogneeClient, line: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(line) {
        Ok(Value::Array(requests)) => {
            let responses = requests
                .into_iter()
                .filter_map(|request| response_for_request(client, request))
                .collect::<Vec<_>>();
            (!responses.is_empty()).then_some(Value::Array(responses))
        }
        Ok(request) => response_for_request(client, request),
        Err(error) => Some(error_response(
            Value::Null,
            PARSE_ERROR,
            format!("Parse error: {error}"),
        )),
    }
}

fn response_for_request(client: &CogneeClient, request: Value) -> Option<Value> {
    let id = request.get("id").cloned()?;
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Some(error_response(
            id,
            INVALID_REQUEST,
            "Invalid request: missing method",
        ));
    };

    match method {
        "initialize" => Some(success_response(
            id,
            initialize_result(request.get("params")),
        )),
        "ping" => Some(success_response(id, json!({}))),
        "tools/list" => Some(success_response(id, tools_list_result(client))),
        "tools/call" => Some(tools_call_response(client, id, request.get("params"))),
        _ => Some(error_response(
            id,
            METHOD_NOT_FOUND,
            format!("Unknown method: {method}"),
        )),
    }
}

fn initialize_result(params: Option<&Value>) -> Value {
    let protocol_version = params
        .and_then(|value| value.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or("2025-11-25");
    json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "cognee-mcp",
            "title": "Cognee Rust MCP",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Use Cognee Rust MCP tools for fast memory recall, search, inspection, remember, forget, and read-model sync. Default recall returns ranked evidence without LLM presummary."
    })
}

fn tools_list_result(client: &CogneeClient) -> Value {
    json!({
        "tools": tools::tool_definitions(client.settings().operator_tools_enabled)
    })
}

fn tools_call_response(client: &CogneeClient, id: Value, params: Option<&Value>) -> Value {
    let Some(name) = params
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
    else {
        return error_response(id, INVALID_PARAMS, "Invalid params: missing tool name");
    };

    let arguments = params
        .and_then(|value| value.get("arguments"))
        .cloned()
        .unwrap_or_else(|| json!({}));

    let packet = match tools::call_tool(client, name, arguments) {
        Ok(packet) => packet,
        Err(error) => markdown_error_packet(name, &error.to_string()),
    };

    success_response(id, content_result(packet))
}

fn content_result(text: String) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ]
    })
}

fn success_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": result
    })
}

fn error_response(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": {
            "code": code,
            "message": message.into()
        }
    })
}

fn markdown_error_packet(tool_name: &str, error: &str) -> String {
    format!(
        "# {title} Failed\n\n\
         ## Answer\n\n\
         The operation failed or was refused.\n\n\
         ## Evidence\n\n\
         {error}\n\n\
         ## Navigate Next\n\n\
         Retry `tools/call` with corrected arguments, or call `get_status` if the Cognee backend state is unclear.\n\n\
         ## Source / Coverage\n\n\
         Tool: `{tool_name}`. Sensitive configuration values were omitted.\n\n\
         Continuation handle status: missing or blocked handles may include `dataset_name`, `dataset_id`, `data_id`, or `pipeline_run_id`.",
        title = title_from_tool_name(tool_name),
        error = error,
        tool_name = tool_name
    )
}

fn title_from_tool_name(tool_name: &str) -> String {
    tool_name
        .split('_')
        .filter(|part| !part.is_empty())
        .map(title_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_word(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn debug_log(message: &str) {
    let Some(path) = env::var_os("COGNEE_MCP_DEBUG_FILE") else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}

fn debug_excerpt(value: &str) -> String {
    value.chars().take(1200).collect()
}

fn mode_name(mode: MessageMode) -> &'static str {
    match mode {
        MessageMode::Framed => "framed",
        MessageMode::Line => "line",
    }
}
