use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_PUBLIC_TOOLS: &[&str] = &[
    "describe",
    "get_status",
    "recall",
    "search",
    "inspect_dataset",
    "inspect_graph",
    "remember",
    "forget",
    "sync_read_model",
];

const OPERATOR_TOOLS: &[&str] = &[
    "add",
    "cognify",
    "improve",
    "manage_schema",
    "manage_ontology",
];
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
        Self::start_with_options(
            "http://localhost:8000",
            operator_tools_enabled,
            None::<PathBuf>,
        )
    }

    fn start_with_service_url_and_read_model(service_url: &str, read_model_path: PathBuf) -> Self {
        Self::start_with_options(service_url, false, Some(read_model_path))
    }

    fn start_with_options(
        service_url: &str,
        operator_tools_enabled: bool,
        read_model_path: Option<PathBuf>,
    ) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cognee-mcp-rs"));
        command.env("COGNEE_SERVICE_URL", service_url);
        command.env(
            "COGNEE_MCP_ENABLE_OPERATOR_TOOLS",
            operator_tools_enabled.to_string(),
        );

        if let Some(path) = read_model_path {
            command.env("COGNEE_MCP_READ_MODEL_PATH", path);
        }

        let mut child = command
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
        match self.responses.recv_timeout(Duration::from_secs(30)) {
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

struct FakeCogneeServer {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl FakeCogneeServer {
    fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("request log lock").clone()
    }
}

fn start_fake_cognee_server() -> FakeCogneeServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fake server should bind");
    let url = format!(
        "http://{}",
        listener.local_addr().expect("fake server address")
    );
    listener
        .set_nonblocking(true)
        .expect("fake server should become nonblocking");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let thread_requests = Arc::clone(&requests);

    thread::spawn(move || {
        loop {
            match listener.accept() {
                Ok((stream, _)) => handle_fake_request(stream, &thread_requests),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }
    });

    FakeCogneeServer { url, requests }
}

fn handle_fake_request(mut stream: TcpStream, requests: &Arc<Mutex<Vec<String>>>) {
    let mut buffer = [0; 4096];
    let Ok(byte_count) = stream.read(&mut buffer) else {
        return;
    };
    let request = String::from_utf8_lossy(&buffer[..byte_count]).to_string();
    let first_line = request.lines().next().unwrap_or_default().to_string();
    requests.lock().expect("request log lock").push(first_line);

    let path = request_path(&request);
    let (status, content_type, body) = fake_response_body(&path);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn request_path(request: &str) -> String {
    request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_string()
}

fn fake_response_body(path: &str) -> (&'static str, &'static str, String) {
    match path {
        "/api/v1/datasets" => (
            "200 OK",
            "application/json",
            json!([{
                "id": "fake-dataset",
                "name": "fake_dataset",
                "createdAt": "2026-05-11T00:00:00Z"
            }])
            .to_string(),
        ),
        "/api/v1/datasets/fake-dataset/graph" => (
            "200 OK",
            "application/json",
            json!({
                "nodes": [
                    {
                        "id": "node-1",
                        "label": "Fleet Router",
                        "type": "concept",
                        "properties": {
                            "description": "Fleet Router coordinates local fastembed embeddings for Cognee memory recall."
                        }
                    },
                    {
                        "id": "node-2",
                        "label": "local fastembed embeddings",
                        "type": "concept",
                        "properties": {
                            "description": "Local embeddings feed the tiny knowledge graph."
                        }
                    }
                ],
                "edges": [
                    {
                        "source": "node-1",
                        "target": "node-2",
                        "label": "uses"
                    }
                ]
            })
            .to_string(),
        ),
        "/api/v1/datasets/fake-dataset/data" => (
            "200 OK",
            "application/json",
            json!([{
                "id": "data-1",
                "name": "cognee-fleet-smoke",
                "mimeType": "text/plain",
                "updatedAt": "2026-05-11T00:00:00Z"
            }])
            .to_string(),
        ),
        "/api/v1/datasets/fake-dataset/data/data-1/raw" => (
            "200 OK",
            "text/plain",
            "Stephen is configuring Cognee to use Fleet Router with local fastembed embeddings. The source describes a tiny knowledge graph for memory recall.".to_string(),
        ),
        "/api/v1/recall" => (
            "200 OK",
            "application/json",
            json!(["Presummary from Python recall baseline."]).to_string(),
        ),
        "/api/v1/remember" => (
            "200 OK",
            "application/json",
            json!({
                "dataset_id": "fake-dataset",
                "items_processed": 1,
                "pipeline_run_id": "fake-pipeline"
            })
            .to_string(),
        ),
        _ => (
            "404 Not Found",
            "application/json",
            json!({ "error": "not found", "path": path }).to_string(),
        ),
    }
}

fn temp_read_model_path(test_name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "cognee-mcp-{test_name}-{}-{stamp}.sqlite",
        std::process::id()
    ))
}

fn seed_fake_read_model(path: &PathBuf) {
    let connection = rusqlite::Connection::open(path).expect("seed read model should open sqlite");
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS datasets(id TEXT PRIMARY KEY, name TEXT, synced_at INTEGER)",
            [],
        )
        .expect("datasets table should seed");
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS data_items(dataset_id TEXT, data_id TEXT, name TEXT, mime TEXT, raw_text TEXT, PRIMARY KEY(dataset_id, data_id))",
            [],
        )
        .expect("data_items table should seed");
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS nodes(dataset_id TEXT, node_id TEXT, label TEXT, node_type TEXT, body TEXT, PRIMARY KEY(dataset_id, node_id))",
            [],
        )
        .expect("nodes table should seed");
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS edges(dataset_id TEXT, source_id TEXT, target_id TEXT, source_label TEXT, target_label TEXT, label TEXT)",
            [],
        )
        .expect("edges table should seed");
    connection
        .execute(
            "INSERT INTO datasets(id, name, synced_at) VALUES ('fake-dataset', 'fake_dataset', 1778522400)",
            [],
        )
        .expect("dataset should seed");
    connection
        .execute(
            "INSERT INTO data_items(dataset_id, data_id, name, mime, raw_text) VALUES ('fake-dataset', 'data-1', 'cognee-fleet-smoke', 'text/plain', 'Stephen is configuring Cognee to use Fleet Router with local fastembed embeddings. The source describes a tiny knowledge graph for memory recall.')",
            [],
        )
        .expect("data item should seed");
    connection
        .execute(
            "INSERT INTO nodes(dataset_id, node_id, label, node_type, body) VALUES ('fake-dataset', 'node-1', 'Fleet Router', 'concept', 'Fleet Router coordinates local fastembed embeddings for Cognee memory recall.')",
            [],
        )
        .expect("node 1 should seed");
    connection
        .execute(
            "INSERT INTO nodes(dataset_id, node_id, label, node_type, body) VALUES ('fake-dataset', 'node-2', 'local fastembed embeddings', 'concept', 'Local embeddings feed the tiny knowledge graph.')",
            [],
        )
        .expect("node 2 should seed");
    connection
        .execute(
            "INSERT INTO edges(dataset_id, source_id, target_id, source_label, target_label, label) VALUES ('fake-dataset', 'node-1', 'node-2', 'Fleet Router', 'local fastembed embeddings', 'uses')",
            [],
        )
        .expect("edge should seed");
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
            "dataset_name": "fleet_smoke_api_20260510"
        }),
        "recall" => json!({
            "query": "What memory exists for the fleet smoke API?",
            "datasets": ["fleet_smoke_api_20260510"],
            "top_k": 3,
            "llm_presummary": false
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
        "remember" => json!({
            "data": ["Contract test explicit memory through Rust MCP."],
            "dataset_name": "fake_dataset",
            "node_set": ["agent_actions"],
            "run_in_background": false
        }),
        "forget" => json!({
            "dataset_name": "fleet_smoke_api_20260510",
            "data_id": "contract-test-placeholder",
            "everything": false,
            "memory_only": true,
            "reason": "contract test targeted forget safety check",
            "confirm": false
        }),
        "sync_read_model" => json!({
            "dataset_name": "fake_dataset"
        }),
        _ => json!({}),
    }
}

#[test]
fn mcp_accepts_claude_code_line_json_protocol() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cognee-mcp-rs"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("compiled MCP binary should launch");

    let mut input = child.stdin.take().expect("MCP stdin should be available");
    let output = child.stdout.take().expect("MCP stdout should be available");
    let mut reader = BufReader::new(output);
    let request = json!({
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {
                "name": "claude-code",
                "version": "contract-test"
            }
        },
        "jsonrpc": "2.0",
        "id": 0
    });

    writeln!(input, "{request}").expect("line JSON request should write");
    input.flush().expect("MCP stdin should flush");

    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .expect("line JSON response should read");
    let response: Value =
        serde_json::from_str(&response_line).expect("line response should be JSON");
    assert_eq!(
        response.pointer("/result/protocolVersion"),
        Some(&json!("2025-11-25"))
    );
    assert_eq!(
        response.pointer("/result/serverInfo/name"),
        Some(&json!("cognee-mcp"))
    );

    drop(input);
    let _ = child.kill();
    let _ = child.wait();
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
    let server = start_fake_cognee_server();
    let read_model_path = temp_read_model_path("default-public-tools");
    seed_fake_read_model(&read_model_path);
    let mut process =
        McpProcess::start_with_service_url_and_read_model(&server.url, read_model_path);
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
fn default_recall_uses_read_model_without_python_recall_endpoint() {
    let server = start_fake_cognee_server();
    let read_model_path = temp_read_model_path("default-recall");
    seed_fake_read_model(&read_model_path);
    let mut process =
        McpProcess::start_with_service_url_and_read_model(&server.url, read_model_path);
    process.initialize();

    let response = process.call_tool(
        "recall",
        json!({
            "query": "How is Fleet Router related to fastembed embeddings?",
            "datasets": ["fake_dataset"],
            "top_k": 5,
            "llm_presummary": false
        }),
    );
    let text = packet_text(&response);

    assert_agent_packet(&text, "recall");
    assert_default_recall_skips_python_recall(&text);
    assert_rust_recall_packet_contract(&text);
    assert!(
        server.requests().is_empty(),
        "default recall must not call any Python/Cognee HTTP endpoint: {:?}",
        server.requests()
    );
}

#[test]
fn default_read_surface_uses_no_backend_http() {
    let server = start_fake_cognee_server();
    let read_model_path = temp_read_model_path("default-agent-surface");
    seed_fake_read_model(&read_model_path);
    let mut process =
        McpProcess::start_with_service_url_and_read_model(&server.url, read_model_path);
    process.initialize();

    let calls = [
        ("describe", json!({})),
        (
            "get_status",
            json!({
                "dataset_name": "fake_dataset"
            }),
        ),
        (
            "search",
            json!({
                "query": "Fleet Router fastembed",
                "datasets": ["fake_dataset"],
                "top_k": 5
            }),
        ),
        (
            "inspect_dataset",
            json!({
                "dataset_name": "fake_dataset",
                "view": "data"
            }),
        ),
        (
            "inspect_dataset",
            json!({
                "data_id": "data-1",
                "view": "raw"
            }),
        ),
        (
            "inspect_graph",
            json!({
                "dataset_name": "fake_dataset"
            }),
        ),
        (
            "forget",
            json!({
                "data_id": "data-1",
                "memory_only": true,
                "reason": "contract test local invalidation"
            }),
        ),
    ];

    for (tool_name, arguments) in calls {
        let response = process.call_tool(tool_name, arguments);
        let text = packet_text(&response);
        assert_agent_packet(&text, tool_name);
    }

    assert!(
        server.requests().is_empty(),
        "default agent surface must not call Python/Cognee HTTP endpoints: {:?}",
        server.requests()
    );
}

#[test]
fn presummary_recall_refuses_without_operator_tools() {
    let server = start_fake_cognee_server();
    let read_model_path = temp_read_model_path("presummary-recall");
    seed_fake_read_model(&read_model_path);
    let mut process =
        McpProcess::start_with_service_url_and_read_model(&server.url, read_model_path);
    process.initialize();

    let response = process.call_tool(
        "recall",
        json!({
            "query": "How is Fleet Router related to fastembed embeddings?",
            "datasets": ["fake_dataset"],
            "top_k": 5,
            "llm_presummary": true
        }),
    );
    let text = packet_text(&response);

    assert_agent_packet(&text, "recall");
    assert!(
        text.contains("operator tools are disabled"),
        "llm_presummary=true should be blocked for the default agent surface: {text}"
    );
    assert!(
        server.requests().is_empty(),
        "blocked presummary must not call Python recall endpoint: {:?}",
        server.requests()
    );
}

#[test]
fn operator_presummary_recall_opts_into_python_recall_endpoint() {
    let server = start_fake_cognee_server();
    let read_model_path = temp_read_model_path("operator-presummary-recall");
    seed_fake_read_model(&read_model_path);
    let mut process = McpProcess::start_with_options(&server.url, true, Some(read_model_path));
    process.initialize();

    let response = process.call_tool(
        "recall",
        json!({
            "query": "How is Fleet Router related to fastembed embeddings?",
            "datasets": ["fake_dataset"],
            "top_k": 5,
            "llm_presummary": true
        }),
    );
    let text = packet_text(&response);

    assert_agent_packet(&text, "recall");
    assert_rust_recall_packet_contract(&text);
    assert!(
        server
            .requests()
            .iter()
            .any(|request| request.starts_with("POST /api/v1/recall ")),
        "llm_presummary=true should call Python recall endpoint: {:?}",
        server.requests()
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
        if tool_name == "recall" {
            assert_default_recall_skips_python_recall(&text);
        }
        assert!(
            text.contains(dataset_name)
                || text.contains(dataset_id)
                || text.contains(data_id)
                || tool_name == "describe"
                || tool_name == "manage_ontology",
            "{tool_name} should preserve the requested live dataset handle: {text}"
        );
    }
}

fn assert_default_recall_skips_python_recall(text: &str) {
    assert!(
        text.contains("Default recall skipped `POST /api/v1/recall`"),
        "default recall should be Rust read-model recall, not Python recall: {text}"
    );
    assert!(
        text.contains("llm_presummary=false"),
        "default recall should document the fast mode: {text}"
    );
}

fn assert_rust_recall_packet_contract(text: &str) {
    for section in [
        "Ranked evidence",
        "Graph relationships",
        "Coverage notes",
        "Confidence:",
        "data_id:",
        "node_id:",
        "edge:",
    ] {
        assert!(
            text.contains(section),
            "recall packet should include `{section}`: {text}"
        );
    }
}
