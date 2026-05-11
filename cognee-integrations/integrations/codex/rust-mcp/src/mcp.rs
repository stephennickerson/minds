use crate::client::CogneeClient;
use crate::tools;
use anyhow::Result;
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};

const JSONRPC_VERSION: &str = "2.0";
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

pub(crate) fn run_mcp(client: CogneeClient) -> Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout();

    while let Some(message) = read_message(&mut reader)? {
        let Some(response) = response_for_line(&client, &message) else {
            continue;
        };
        write_message(&mut stdout, &response)?;
    }

    Ok(())
}

fn read_message(reader: &mut BufReader<impl Read>) -> Result<Option<String>> {
    let mut first_line = String::new();
    if reader.read_line(&mut first_line)? == 0 {
        return Ok(None);
    }
    if first_line.trim().is_empty() {
        return read_message(reader);
    }
    if first_line.starts_with("Content-Length:") {
        return framed_message(reader, &first_line).map(Some);
    }
    Ok(Some(first_line))
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

fn write_message(stdout: &mut impl Write, response: &Value) -> Result<()> {
    let body = serde_json::to_vec(response)?;
    write!(stdout, "Content-Length: {}\r\n\r\n", body.len())?;
    stdout.write_all(&body)?;
    stdout.flush()?;
    Ok(())
}

fn response_for_line(client: &CogneeClient, line: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(line) {
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
        "initialize" => Some(success_response(id, initialize_result())),
        "tools/list" => Some(success_response(id, tools_list_result(client))),
        "tools/call" => Some(tools_call_response(client, id, request.get("params"))),
        _ => Some(error_response(
            id,
            METHOD_NOT_FOUND,
            format!("Unknown method: {method}"),
        )),
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "cognee-rust-mcp",
            "version": env!("CARGO_PKG_VERSION")
        }
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
