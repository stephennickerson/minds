use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

const DEFAULT_PUBLIC_TOOLS: &[&str] = &[
    "describe",
    "get_status",
    "remember",
    "add",
    "cognify",
    "recall",
    "search",
    "inspect_dataset",
    "inspect_graph",
    "improve",
    "forget",
];

const OPERATOR_TOOLS: &[&str] = &["manage_schema", "manage_ontology"];
const REQUIRED_PACKET_SECTIONS: &[&str] = &[
    "## Answer",
    "## Evidence",
    "## Navigate Next",
    "## Source / Coverage",
];

struct McpProcess {
    child: Child,
    input: ChildStdin,
    responses: Receiver<Result<Value, String>>,
    next_id: u64,
}

impl McpProcess {
    fn start() -> Self {
        Self::start_with_operator_tools(false)
    }

    fn start_with_operator_tools(operator_tools_enabled: bool) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_cognee-mcp-rs"))
            .env("COGNEE_SERVICE_URL", "http://localhost:8000")
            .env(
                "COGNEE_MCP_ENABLE_OPERATOR_TOOLS",
                operator_tools_enabled.to_string(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("compiled MCP binary should launch");

        let input = child.stdin.take().expect("MCP stdin should be available");
        let output = child.stdout.take().expect("MCP stdout should be available");
        let (sender, responses) = mpsc::channel();

        thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let message = read_framed_json(&mut reader);
                let should_stop = message.is_err();
                if sender.send(message).is_err() || should_stop {
                    break;
                }
            }
        });

        Self {
            child,
            input,
            responses,
            next_id: 1,
        }
    }

    fn initialize(&mut self) {
        let response = self.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "cognee-rust-mcp-contract-tests",
                    "version": "0.1.0"
                }
            }),
        );
        assert!(
            response.get("result").is_some(),
            "initialize should return a JSON-RPC result: {response:#}"
        );
        self.notify("notifications/initialized", json!({}));
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&request);

        loop {
            let response = self.read_response();
            if response.get("id").and_then(Value::as_u64) == Some(id) {
                return response;
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.write_message(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }));
    }

    fn write_message(&mut self, message: &Value) {
        let body = serde_json::to_vec(message).expect("JSON-RPC message should serialize");
        write!(self.input, "Content-Length: {}\r\n\r\n", body.len())
            .expect("MCP header should write to stdin");
        self.input
            .write_all(&body)
            .expect("MCP body should write to stdin");
        self.input.flush().expect("MCP stdin should flush");
    }

    fn read_response(&self) -> Value {
        match self.responses.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(response)) => response,
            Ok(Err(message)) => {
                panic!("MCP stdout did not contain a framed JSON-RPC response: {message}")
            }
            Err(_) => panic!("timed out waiting for MCP JSON-RPC response"),
        }
    }

    fn list_tools(&mut self) -> Vec<Value> {
        let response = self.request("tools/list", json!({}));
        response
            .pointer("/result/tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_else(|| panic!("tools/list should return result.tools: {response:#}"))
    }

    fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
            }),
        )
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_framed_json(reader: &mut BufReader<impl Read>) -> Result<Value, String> {
    let mut content_length = None;

    loop {
        let mut line = String::new();
        let byte_count = reader
            .read_line(&mut line)
            .map_err(|error| format!("failed to read stdout header: {error}"))?;
        if byte_count == 0 {
            return Err("stdout closed before a response was received".to_string());
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }

        let Some((name, value)) = trimmed.split_once(':') else {
            return Err(format!("unexpected stdout before MCP frame: {trimmed}"));
        };

        if name.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| format!("invalid Content-Length value `{value}`: {error}"))?,
            );
        }
    }

    let content_length =
        content_length.ok_or_else(|| "missing Content-Length header".to_string())?;
    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .map_err(|error| format!("failed to read response body: {error}"))?;
    serde_json::from_slice(&body).map_err(|error| format!("response body was not JSON: {error}"))
}

fn tool_names(tools: &[Value]) -> Vec<String> {
    tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect()
}

fn packet_text(response: &Value) -> String {
    response
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("tools/call should return result.content[0].text: {response:#}"))
        .to_string()
}

fn assert_agent_packet(text: &str, tool_name: &str) {
    let trimmed = text.trim_start();
    assert!(
        !trimmed.starts_with('{') && !trimmed.starts_with('['),
        "{tool_name} should not return raw JSON as primary text: {text}"
    );

    for section in REQUIRED_PACKET_SECTIONS {
        assert!(
            text.contains(section),
            "{tool_name} packet should include `{section}`: {text}"
        );
    }

    assert!(
        contains_continuation_handle_or_missing_handle_note(text),
        "{tool_name} packet should include a continuation handle or explain what handle is missing: {text}"
    );
    assert!(
        names_exact_next_tool_call(text),
        "{tool_name} packet should name an exact next tool call: {text}"
    );
}

fn contains_continuation_handle_or_missing_handle_note(text: &str) -> bool {
    [
        "dataset_name",
        "dataset_id",
        "data_id",
        "pipeline_run_id",
        "search_type",
        "node_name",
        "ontology_key",
        "schema_id",
        "source file",
        "raw data",
        "missing",
        "No handle",
        "no handle",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn names_exact_next_tool_call(text: &str) -> bool {
    DEFAULT_PUBLIC_TOOLS.iter().any(|tool_name| {
        text.contains(&format!("`{tool_name}`")) || text.contains(&format!("{tool_name}("))
    })
}

fn contract_tool_arguments(tool_name: &str) -> Value {
    match tool_name {
        "describe" => json!({}),
        "get_status" => json!({
            "dataset_name": "fleet_smoke_api_20260510",
            "include_detailed_health": false
        }),
        "remember" => json!({
            "dataset_name": "fleet_smoke_api_20260510"
        }),
        "add" => json!({
            "dataset_name": "fleet_smoke_api_20260510"
        }),
        "cognify" => json!({
            "run_in_background": true
        }),
        "recall" => json!({
            "query": "What memory exists for the fleet smoke API?",
            "datasets": ["fleet_smoke_api_20260510"],
            "top_k": 3,
            "only_context": false
        }),
        "search" => json!({
            "query": "fleet smoke API",
            "search_type": "CHUNKS",
            "datasets": ["fleet_smoke_api_20260510"],
            "top_k": 3,
            "only_context": true
        }),
        "inspect_dataset" => json!({
            "dataset_name": "fleet_smoke_api_20260510",
            "view": "summary",
            "limit": 10
        }),
        "inspect_graph" => json!({
            "dataset_name": "fleet_smoke_api_20260510",
            "limit": 10
        }),
        "improve" => json!({
            "node_name": [],
            "extraction_tasks": [],
            "enrichment_tasks": [],
            "run_in_background": true
        }),
        "forget" => json!({
            "dataset_name": "fleet_smoke_api_20260510",
            "data_id": "contract-test-placeholder",
            "everything": false,
            "memory_only": true,
            "reason": "contract test targeted forget safety check",
            "confirm": false
        }),
        _ => json!({}),
    }
}

#[test]
fn default_tool_surface_exposes_only_agent_tools() {
    let mut process = McpProcess::start();
    process.initialize();

    let names = tool_names(&process.list_tools());
    assert_eq!(names, DEFAULT_PUBLIC_TOOLS);
    for operator_tool in OPERATOR_TOOLS {
        assert!(
            !names.iter().any(|name| name == operator_tool),
            "{operator_tool} should be hidden by default"
        );
    }
}

#[test]
fn every_default_public_tool_returns_an_agent_packet() {
    let mut process = McpProcess::start();
    process.initialize();

    let names = tool_names(&process.list_tools());
    assert_eq!(names, DEFAULT_PUBLIC_TOOLS);

    for tool_name in DEFAULT_PUBLIC_TOOLS {
        let response = process.call_tool(tool_name, contract_tool_arguments(tool_name));
        let text = packet_text(&response);
        assert_agent_packet(&text, tool_name);
    }
}

#[test]
fn broad_forget_refuses_without_destructive_environment_gate() {
    let mut process = McpProcess::start();
    process.initialize();

    let response = process.call_tool(
        "forget",
        json!({
            "dataset_name": "fleet_smoke_api_20260510",
            "everything": true,
            "reason": "contract test broad delete refusal",
            "confirm": true
        }),
    );
    let text = packet_text(&response);

    assert_agent_packet(&text, "forget");
    assert!(
        text.to_lowercase().contains("refus") || text.to_lowercase().contains("destructive"),
        "broad forget should refuse when COGNEE_MCP_ENABLE_DESTRUCTIVE_TOOLS is not true: {text}"
    );
}

#[test]
fn live_read_surfaces_work_against_local_cognee_when_enabled() {
    if std::env::var("COGNEE_MCP_LIVE_TESTS").as_deref() != Ok("true") {
        eprintln!(
            "skipping live Cognee MCP assertions; set COGNEE_MCP_LIVE_TESTS=true to run them"
        );
        return;
    }

    let mut process = McpProcess::start_with_operator_tools(true);
    process.initialize();

    let dataset_name = "fleet_smoke_api_20260510";
    let dataset_id = "40089743-cf53-50f0-bf25-3ce347e8d6d7";
    let data_id = "1eedb6e9-e470-51ea-853d-da231b71bca6";
    let live_calls = [
        ("describe", json!({})),
        (
            "get_status",
            json!({
                "dataset_name": dataset_name,
                "include_detailed_health": false
            }),
        ),
        (
            "inspect_dataset",
            json!({
                "dataset_name": dataset_name,
                "view": "summary",
                "limit": 10
            }),
        ),
        (
            "inspect_dataset",
            json!({
                "dataset_name": dataset_name,
                "view": "data",
                "limit": 10
            }),
        ),
        (
            "inspect_dataset",
            json!({
                "dataset_id": dataset_id,
                "view": "schema"
            }),
        ),
        (
            "inspect_dataset",
            json!({
                "dataset_id": dataset_id,
                "data_id": data_id,
                "view": "raw"
            }),
        ),
        (
            "inspect_graph",
            json!({
                "dataset_name": dataset_name,
                "limit": 5
            }),
        ),
        (
            "search",
            json!({
                "query": "fleet smoke API",
                "search_type": "CHUNKS",
                "datasets": [dataset_name],
                "top_k": 5,
                "only_context": true
            }),
        ),
        (
            "recall",
            json!({
                "query": "What does the fleet smoke API dataset contain?",
                "datasets": [dataset_name],
                "top_k": 5,
                "only_context": false
            }),
        ),
        (
            "manage_schema",
            json!({
                "dataset_name": dataset_name
            }),
        ),
        ("manage_ontology", json!({})),
    ];

    for (tool_name, arguments) in live_calls {
        let response = process.call_tool(tool_name, arguments);
        let text = packet_text(&response);
        assert_agent_packet(&text, tool_name);
        assert!(
            text.contains(dataset_name)
                || text.contains(dataset_id)
                || tool_name == "describe"
                || tool_name == "manage_ontology",
            "{tool_name} should preserve the requested live dataset handle: {text}"
        );
    }
}
