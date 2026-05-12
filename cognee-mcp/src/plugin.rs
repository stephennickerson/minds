use crate::client::CogneeClient;
use crate::read_model::{ReadModel, RecallRead};
use crate::tools;
use anyhow::{Context, Result};
use clap::Subcommand;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum HookCommand {
    SessionStart,
    ContextLookup,
    StoreUserPrompt,
    StoreToolUse,
    StoreStop,
    PreCompact,
    SessionEnd {
        #[arg(long)]
        detached: bool,
    },
}

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum DaemonCommand {
    IdleWatcher {
        #[arg(long)]
        once: bool,
    },
}

pub(crate) fn run_hook(client: &CogneeClient, command: HookCommand) -> Result<()> {
    match command {
        HookCommand::SessionStart => session_start(),
        HookCommand::ContextLookup => context_lookup(client),
        HookCommand::StoreUserPrompt => store_user_prompt(client),
        HookCommand::StoreToolUse => store_tool_use(client),
        HookCommand::StoreStop => store_stop(client),
        HookCommand::PreCompact => pre_compact(client),
        HookCommand::SessionEnd { detached } => session_end(client, detached),
    }
}

pub(crate) fn run_daemon(client: &CogneeClient, command: DaemonCommand) -> Result<()> {
    match command {
        DaemonCommand::IdleWatcher { once } => idle_watcher(client, once),
    }
}

pub(crate) fn print_status_line() -> Result<()> {
    let state = PluginState::new()?;
    let resolved = state.resolved();
    let session_id = string_field(&resolved, "session_id");
    let dataset = fallback_dash(&string_field(&resolved, "dataset"));
    let suffix = fallback_dash(session_id.rsplit('_').next().unwrap_or(""));
    let recall = status_recall(&state);
    let saves = status_saves(&state, &session_id);
    println!(
        "{}",
        join_non_empty(
            &[
                format!("cognee[rust] ds={dataset} sess={suffix}"),
                recall,
                saves
            ],
            " | "
        )
    );
    Ok(())
}

fn session_start() -> Result<()> {
    let state = PluginState::new()?;
    let config = PluginConfig::load(&state);
    let cwd = claude_cwd();
    let session_id = session_id(&config, &cwd);
    state.write_resolved(&config, &session_id, &cwd)?;
    state.touch_activity()?;
    if !idle_disabled() {
        spawn_idle_watcher(&state)?;
    }
    print_hook_json("SessionStart", &session_start_message(&config, &session_id));
    Ok(())
}

fn context_lookup(client: &CogneeClient) -> Result<()> {
    let state = PluginState::new()?;
    let payload = stdin_json()?;
    let prompt = string_field(&payload, "prompt");
    if prompt.trim().len() < 5 {
        return Ok(());
    }
    let config = PluginConfig::load(&state);
    let resolved = state.ensure_resolved(&config)?;
    let session_id = string_field(&resolved, "session_id");
    let saves = state.read_and_reset_saves(&session_id)?;
    let session_hits = state.matching_events(&session_id, &prompt, "prompt", config.top_k)?;
    let trace_hits = state.matching_events(&session_id, &prompt, "trace", config.top_k)?;
    let graph = rust_recall(client, &prompt, &config.dataset, config.top_k);
    let context = context_packet(&prompt, &session_hits, &trace_hits, graph.as_ref());
    let counts = hit_counts(&session_hits, &trace_hits, graph.as_ref());
    let header = recall_header(&counts, &saves);
    state.write_last_recall(&session_id, &counts, &saves)?;
    state.append_audit(&session_id, &prompt, &counts, &context)?;
    print_user_prompt_output(&header, &format!("{header}\n\n{context}"));
    Ok(())
}

fn store_user_prompt(client: &CogneeClient) -> Result<()> {
    let payload = stdin_json()?;
    let prompt = string_field(&payload, "prompt");
    if prompt.trim().len() < 5 {
        return Ok(());
    }
    store_entry(
        client,
        "prompt",
        prompt.clone(),
        json!({ "type": "qa", "question": prompt, "answer": "", "context": "" }),
    )
}

fn store_tool_use(client: &CogneeClient) -> Result<()> {
    let payload = stdin_json()?;
    let tool_name = fallback_unknown(&string_field(&payload, "tool_name"));
    if is_self_cognee_bash(&payload, &tool_name) {
        return Ok(());
    }
    let status = tool_status(&payload);
    let output = compact_json(
        payload
            .get("tool_output")
            .or_else(|| payload.get("tool_response")),
    );
    let params = compact_json(payload.get("tool_input"));
    let text = format!("{tool_name} [{status}]\nParams: {params}\nReturn: {output}");
    store_entry(
        client,
        "trace",
        text.clone(),
        json!({
            "type": "trace",
            "origin_function": tool_name,
            "status": status,
            "method_params": { "value": params },
            "method_return_value": output,
            "error_message": "",
            "generate_feedback_with_llm": false
        }),
    )
}

fn store_stop(client: &CogneeClient) -> Result<()> {
    let payload = stdin_json()?;
    let message = first_non_empty(&[
        string_field(&payload, "assistant_message"),
        string_field(&payload, "last_assistant_message"),
    ]);
    if message.trim().is_empty() || message == "null" {
        return Ok(());
    }
    store_entry(
        client,
        "answer",
        message.clone(),
        json!({ "type": "qa", "question": "", "answer": message, "context": "" }),
    )
}

fn pre_compact(client: &CogneeClient) -> Result<()> {
    let state = PluginState::new()?;
    let config = PluginConfig::load(&state);
    let resolved = state.ensure_resolved(&config)?;
    let session_id = string_field(&resolved, "session_id");
    let recent = state.recent_events(&session_id, 12)?;
    let query = query_from_events(&recent);
    let graph = (!query.is_empty())
        .then(|| rust_recall(client, &query, &config.dataset, 5))
        .flatten();
    let anchor = precompact_anchor(&recent, graph.as_ref());
    if !anchor.trim().is_empty() {
        println!("{anchor}");
    }
    Ok(())
}

fn session_end(client: &CogneeClient, detached: bool) -> Result<()> {
    if !detached {
        spawn_session_end_worker()?;
        return Ok(());
    }
    bridge_session(client)
}

fn idle_watcher(client: &CogneeClient, once: bool) -> Result<()> {
    let state = PluginState::new()?;
    state.write_pid()?;
    let result = if once {
        bridge_session(client)
    } else {
        idle_loop(client, &state)
    };
    state.remove_pid();
    result
}

fn store_entry(client: &CogneeClient, kind: &str, text: String, entry: Value) -> Result<()> {
    let state = PluginState::new()?;
    let config = PluginConfig::load(&state);
    let resolved = state.ensure_resolved(&config)?;
    let session_id = string_field(&resolved, "session_id");
    let dataset = string_field(&resolved, "dataset");
    state.append_event(&session_id, &dataset, kind, &text, &entry)?;
    state.bump_save_counter(&session_id, save_kind(kind))?;
    state.touch_activity()?;
    let _ = remember_entry(client, &dataset, &session_id, entry)
        .map_err(|error| state.hook_log("remember_entry_failed", &error.to_string()));
    Ok(())
}

fn bridge_session(client: &CogneeClient) -> Result<()> {
    let state = PluginState::new()?;
    let config = PluginConfig::load(&state);
    let resolved = state.ensure_resolved(&config)?;
    let session_id = string_field(&resolved, "session_id");
    let dataset = string_field(&resolved, "dataset");
    let document = state.session_document(&session_id)?;
    if document.trim().is_empty() {
        state.hook_log("bridge_skipped_empty", &session_id);
        return Ok(());
    }
    let remember_args = json!({
        "data": [document],
        "dataset_name": dataset,
        "node_set": ["agent_actions"],
        "run_in_background": false
    });
    let _ = tools::call_tool(client, "remember", remember_args)
        .map_err(|error| state.hook_log("bridge_remember_failed", &error.to_string()));
    let _ = tools::call_tool(
        client,
        "sync_read_model",
        json!({ "dataset_name": dataset }),
    )
    .map_err(|error| state.hook_log("bridge_sync_failed", &error.to_string()));
    state.hook_log("bridge_done", &session_id);
    Ok(())
}

fn idle_loop(client: &CogneeClient, state: &PluginState) -> Result<()> {
    let poll = env_float("COGNEE_IDLE_POLL", 10.0);
    let threshold = env_float("COGNEE_IDLE_THRESHOLD", 60.0);
    loop {
        if state.stop_file().exists() {
            return bridge_session(client);
        }
        if state.idle_seconds()? >= threshold {
            return bridge_session(client);
        }
        thread::sleep(Duration::from_secs_f64(poll.max(1.0)));
    }
}

fn rust_recall(
    client: &CogneeClient,
    query: &str,
    dataset: &str,
    top_k: usize,
) -> Option<RecallRead> {
    let model = ReadModel::from_client(client);
    let with_dataset =
        json!({ "query": query, "datasets": [dataset], "top_k": top_k, "llm_presummary": false });
    let without_dataset = json!({ "query": query, "top_k": top_k, "llm_presummary": false });
    if !dataset.trim().is_empty()
        && let Ok(read) = model.recall(&with_dataset)
    {
        return Some(read);
    }
    model.recall(&without_dataset).ok()
}

fn remember_entry(
    client: &CogneeClient,
    dataset: &str,
    session_id: &str,
    entry: Value,
) -> Result<Value> {
    client.post_json(
        "/api/v1/remember/entry",
        &json!({ "entry": entry, "dataset_name": dataset, "session_id": session_id }),
    )
}

fn context_packet(
    prompt: &str,
    session_hits: &[Value],
    trace_hits: &[Value],
    graph: Option<&RecallRead>,
) -> String {
    let mut sections = Vec::new();
    sections.push(format!("Relevant Rust memory for: `{prompt}`"));
    if let Some(graph) = graph {
        let graph_context = format_graph_context(graph);
        if !graph_context.trim().is_empty() {
            sections.push(graph_context);
        }
    }
    if !trace_hits.is_empty() {
        sections.push(format_events("Prior agent trace", trace_hits));
    }
    if !session_hits.is_empty() {
        sections.push(format_events("Prior session turns", session_hits));
    }
    if sections.len() == 1 {
        sections.push("(no memory matches for this prompt)".to_string());
    }
    sections.join("\n\n")
}

fn format_graph_context(graph: &RecallRead) -> String {
    let evidence = graph
        .evidence
        .iter()
        .take(5)
        .map(|item| {
            format!(
                "- [{}] {} ({}) {}",
                item.rank, item.label, item.handle, item.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let relationships = graph
        .relationships
        .iter()
        .take(5)
        .map(|item| {
            format!(
                "- {} --{}--> {} ({})",
                item.source, item.relationship, item.target, item.handle
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    join_non_empty(
        &[
            titled_block("Knowledge graph evidence", &evidence),
            titled_block("Graph relationships", &relationships),
        ],
        "\n\n",
    )
}

fn titled_block(title: &str, body: &str) -> String {
    if body.trim().is_empty() {
        String::new()
    } else {
        format!("### {title}\n{body}")
    }
}

fn format_events(title: &str, events: &[Value]) -> String {
    let body = events
        .iter()
        .map(|event| format!("- {}", truncate(&string_field(event, "text"), 500)))
        .collect::<Vec<_>>()
        .join("\n");
    format!("### {title}\n{body}")
}

fn precompact_anchor(events: &[Value], graph: Option<&RecallRead>) -> String {
    let mut sections = vec![
        "## Cognee Memory Anchor".to_string(),
        "Preserved context from Rust session state and read-model recall.".to_string(),
    ];
    if !events.is_empty() {
        sections.push(format_events("Recent session memory", events));
    }
    if let Some(graph) = graph {
        let graph_context = format_graph_context(graph);
        if !graph_context.trim().is_empty() {
            sections.push(graph_context);
        }
    }
    if sections.len() <= 2 {
        String::new()
    } else {
        sections.join("\n\n")
    }
}

fn hit_counts(session_hits: &[Value], trace_hits: &[Value], graph: Option<&RecallRead>) -> Value {
    json!({
        "session": session_hits.len(),
        "trace": trace_hits.len(),
        "graph_context": graph.map(|read| read.evidence.len() + read.relationships.len()).unwrap_or(0)
    })
}

fn recall_header(counts: &Value, saves: &Value) -> String {
    format!(
        "cognee rust recall: {} session / {} trace / {} graph hits | saves last turn: {} prompt / {} trace / {} answer",
        number_field(counts, "session"),
        number_field(counts, "trace"),
        number_field(counts, "graph_context"),
        number_field(saves, "prompt"),
        number_field(saves, "trace"),
        number_field(saves, "answer"),
    )
}

fn session_start_message(config: &PluginConfig, session_id: &str) -> String {
    format!(
        "## Cognee Memory Connected\nMode: rust | Dataset: {} | Session: {}\n\nUse Rust MCP tools for explicit memory actions. Hooks now run through `cognee-mcp-rs hook ...`.",
        config.dataset, session_id
    )
}

fn print_hook_json(event_name: &str, message: &str) {
    println!(
        "{}",
        json!({
            "hookSpecificOutput": {
                "hookEventName": event_name,
                "systemMessage": message
            }
        })
    );
}

fn print_user_prompt_output(header: &str, context: &str) {
    println!(
        "{}",
        json!({
            "hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": context,
                "systemMessage": header
            }
        })
    );
}

fn spawn_idle_watcher(state: &PluginState) -> Result<()> {
    remove_file_if_exists(&state.stop_file());
    let log = state.open_log("watcher.log")?;
    let err = log.try_clone()?;
    Command::new(env::current_exe()?)
        .args(["daemon", "idle-watcher"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err))
        .spawn()
        .context("spawn idle watcher")?;
    Ok(())
}

fn spawn_session_end_worker() -> Result<()> {
    Command::new(env::current_exe()?)
        .args(["hook", "session-end", "--detached"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn session-end worker")?;
    Ok(())
}

#[derive(Clone)]
struct PluginConfig {
    dataset: String,
    session_strategy: String,
    session_prefix: String,
    top_k: usize,
}

impl PluginConfig {
    fn load(state: &PluginState) -> Self {
        let file = state.config();
        Self {
            dataset: env_or_file("COGNEE_PLUGIN_DATASET", &file, "dataset", "claude_sessions"),
            session_strategy: env_or_file(
                "COGNEE_SESSION_STRATEGY",
                &file,
                "session_strategy",
                "per-directory",
            ),
            session_prefix: env_or_file("COGNEE_SESSION_PREFIX", &file, "session_prefix", "cc"),
            top_k: env_or_file("COGNEE_TOP_K", &file, "top_k", "5")
                .parse::<usize>()
                .unwrap_or(5)
                .clamp(1, 20),
        }
    }
}

struct PluginState {
    dir: PathBuf,
}

impl PluginState {
    fn new() -> Result<Self> {
        let dir = env::var_os("COGNEE_PLUGIN_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".cognee-plugin"));
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn config(&self) -> Value {
        read_json(&self.path("config.json"))
    }

    fn resolved(&self) -> Value {
        read_json(&self.path("resolved.json"))
    }

    fn ensure_resolved(&self, config: &PluginConfig) -> Result<Value> {
        let current = self.resolved();
        if !string_field(&current, "session_id").is_empty() {
            return Ok(current);
        }
        let cwd = claude_cwd();
        let session_id = session_id(config, &cwd);
        self.write_resolved(config, &session_id, &cwd)?;
        Ok(self.resolved())
    }

    fn write_resolved(&self, config: &PluginConfig, session_id: &str, cwd: &str) -> Result<()> {
        write_json(
            &self.path("resolved.json"),
            &json!({
                "session_id": session_id,
                "dataset": config.dataset,
                "user_id": "",
                "cwd": cwd,
                "api_key": env::var("COGNEE_API_KEY").unwrap_or_default()
            }),
        )
    }

    fn append_event(
        &self,
        session_id: &str,
        dataset: &str,
        kind: &str,
        text: &str,
        payload: &Value,
    ) -> Result<()> {
        let event = json!({
            "ts": unix_timestamp(),
            "session_id": session_id,
            "dataset": dataset,
            "kind": kind,
            "text": truncate(text, 4000),
            "payload": payload
        });
        append_line(&self.path("session_events.jsonl"), &event.to_string())
    }

    fn matching_events(
        &self,
        session_id: &str,
        query: &str,
        kind: &str,
        limit: usize,
    ) -> Result<Vec<Value>> {
        let words = query_words(query);
        let mut scored = self
            .events(session_id)?
            .into_iter()
            .filter(|event| string_field(event, "kind") == kind)
            .map(|event| (score_text(&string_field(&event, "text"), &words), event))
            .filter(|(score, _)| *score > 0)
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| right.0.cmp(&left.0));
        Ok(scored
            .into_iter()
            .take(limit)
            .map(|(_, event)| event)
            .collect())
    }

    fn recent_events(&self, session_id: &str, limit: usize) -> Result<Vec<Value>> {
        let mut events = self.events(session_id)?;
        events.reverse();
        events.truncate(limit);
        events.reverse();
        Ok(events)
    }

    fn events(&self, session_id: &str) -> Result<Vec<Value>> {
        let path = self.path("session_events.jsonl");
        let body = fs::read_to_string(path).unwrap_or_default();
        Ok(body
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|value| string_field(value, "session_id") == session_id)
            .collect())
    }

    fn session_document(&self, session_id: &str) -> Result<String> {
        let events = self.events(session_id)?;
        Ok(events
            .iter()
            .map(|event| {
                format!(
                    "{}: {}",
                    string_field(event, "kind"),
                    string_field(event, "text")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n"))
    }

    fn bump_save_counter(&self, session_id: &str, kind: &str) -> Result<()> {
        let path = self.path("save_counter.json");
        let mut data = read_json(&path);
        ensure_object(&mut data);
        let current = data
            .pointer(&format!("/{session_id}/{kind}"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
            + 1;
        ensure_object(&mut data[session_id]);
        data[session_id][kind] = json!(current);
        write_json(&path, &data)
    }

    fn read_and_reset_saves(&self, session_id: &str) -> Result<Value> {
        let path = self.path("save_counter.json");
        let mut data = read_json(&path);
        ensure_object(&mut data);
        let saves = data.get(session_id).cloned().unwrap_or_else(zero_saves);
        data[session_id] = zero_saves();
        write_json(&path, &data)?;
        Ok(saves)
    }

    fn write_last_recall(&self, session_id: &str, counts: &Value, saves: &Value) -> Result<()> {
        write_json(
            &self.path("last_recall.json"),
            &json!({ "session_id": session_id, "ts": unix_timestamp(), "hits": counts, "saves_last_turn": saves }),
        )
    }

    fn append_audit(
        &self,
        session_id: &str,
        prompt: &str,
        counts: &Value,
        context: &str,
    ) -> Result<()> {
        append_line(
            &self.path("recall-audit.log"),
            &json!({ "ts": unix_timestamp(), "session_id": session_id, "prompt": prompt, "hits": counts, "context": context }).to_string(),
        )
    }

    fn touch_activity(&self) -> Result<()> {
        fs::write(self.path("activity.ts"), unix_timestamp().to_string())?;
        Ok(())
    }

    fn idle_seconds(&self) -> Result<f64> {
        let text = fs::read_to_string(self.path("activity.ts")).unwrap_or_default();
        let last = text
            .trim()
            .parse::<f64>()
            .unwrap_or_else(|_| unix_timestamp() as f64);
        Ok(unix_timestamp() as f64 - last)
    }

    fn write_pid(&self) -> Result<()> {
        fs::write(self.path("watcher.pid"), std::process::id().to_string())?;
        Ok(())
    }

    fn remove_pid(&self) {
        remove_file_if_exists(&self.path("watcher.pid"));
    }

    fn hook_log(&self, event: &str, detail: &str) {
        let _ = append_line(
            &self.path("hook.log"),
            &json!({ "ts": unix_timestamp(), "event": event, "detail": detail }).to_string(),
        );
    }

    fn open_log(&self, name: &str) -> io::Result<File> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path(name))
    }

    fn stop_file(&self) -> PathBuf {
        self.path("watcher.stop")
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

fn env_or_file(env_name: &str, file: &Value, key: &str, fallback: &str) -> String {
    env::var(env_name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| file.get(key).and_then(value_to_string))
        .unwrap_or_else(|| fallback.to_string())
}

fn value_to_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(ToString::to_string)
        .or_else(|| value.as_u64().map(|number| number.to_string()))
}

fn session_id(config: &PluginConfig, cwd: &str) -> String {
    if let Ok(value) = env::var("COGNEE_SESSION_ID")
        && !value.trim().is_empty()
    {
        return value;
    }
    if config.session_strategy == "static" {
        return format!("{}_session", config.session_prefix);
    }
    let hash = short_sha256(cwd);
    let name = Path::new(cwd)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("workspace");
    format!("{}_{}_{}", config.session_prefix, sanitize_id(name), hash)
}

fn short_sha256(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
        .chars()
        .take(12)
        .collect()
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn stdin_json() -> Result<Value> {
    let mut body = String::new();
    io::stdin().read_to_string(&mut body)?;
    if body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&body).context("parse hook stdin JSON")
}

fn read_json(path: &Path) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    create_parent(path)?;
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn append_line(path: &Path, line: &str) -> Result<()> {
    create_parent(path)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn claude_cwd() -> String {
    env::var("CLAUDE_CWD")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::current_dir()
                .ok()
                .map(|path| path.display().to_string())
        })
        .unwrap_or_else(|| ".".to_string())
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn number_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn zero_saves() -> Value {
    json!({ "prompt": 0, "trace": 0, "answer": 0 })
}

fn ensure_object(value: &mut Value) {
    if !value.is_object() {
        *value = json!({});
    }
}

fn query_words(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|word| word.len() >= 3)
        .map(|word| word.to_ascii_lowercase())
        .collect()
}

fn score_text(text: &str, words: &[String]) -> u64 {
    let lower = text.to_ascii_lowercase();
    words
        .iter()
        .filter(|word| lower.contains(word.as_str()))
        .count() as u64
}

fn query_from_events(events: &[Value]) -> String {
    events
        .iter()
        .rev()
        .take(4)
        .map(|event| string_field(event, "text"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn compact_json(value: Option<&Value>) -> String {
    let text = value
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .unwrap_or_else(|| value.to_string())
        })
        .unwrap_or_default();
    truncate(&text, 4000)
}

fn truncate(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(limit).collect::<String>();
    if chars.next().is_none() {
        value.to_string()
    } else {
        format!("{truncated}...[truncated]")
    }
}

fn tool_status(payload: &Value) -> String {
    let response = payload
        .get("tool_response")
        .or_else(|| payload.get("tool_output"));
    match response {
        Some(Value::Object(map)) if map.get("is_error").and_then(Value::as_bool) == Some(true) => {
            "error".to_string()
        }
        _ if !string_field(payload, "error").is_empty() => "error".to_string(),
        _ => "success".to_string(),
    }
}

fn is_self_cognee_bash(payload: &Value, tool_name: &str) -> bool {
    if tool_name != "Bash" {
        return false;
    }
    payload
        .pointer("/tool_input/command")
        .and_then(Value::as_str)
        .map(|command| command.to_ascii_lowercase().contains("cognee"))
        .unwrap_or(false)
}

fn save_kind(kind: &str) -> &str {
    match kind {
        "prompt" => "prompt",
        "answer" => "answer",
        _ => "trace",
    }
}

fn first_non_empty(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

fn fallback_unknown(value: &str) -> String {
    if value.trim().is_empty() {
        "unknown".to_string()
    } else {
        value.to_string()
    }
}

fn fallback_dash(value: &str) -> String {
    if value.trim().is_empty() {
        "-".to_string()
    } else {
        value.to_string()
    }
}

fn join_non_empty(values: &[String], separator: &str) -> String {
    values
        .iter()
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(separator)
}

fn status_recall(state: &PluginState) -> String {
    let last = read_json(&state.path("last_recall.json"));
    let hits = last.get("hits").cloned().unwrap_or_else(|| json!({}));
    format!(
        "recall: {}s/{}t/{}g",
        number_field(&hits, "session"),
        number_field(&hits, "trace"),
        number_field(&hits, "graph_context")
    )
}

fn status_saves(state: &PluginState, session_id: &str) -> String {
    let saves = read_json(&state.path("save_counter.json"))
        .get(session_id)
        .cloned()
        .unwrap_or_else(zero_saves);
    let total = number_field(&saves, "prompt")
        + number_field(&saves, "trace")
        + number_field(&saves, "answer");
    if total == 0 {
        String::new()
    } else {
        format!(
            "saving: {}p/{}t/{}a",
            number_field(&saves, "prompt"),
            number_field(&saves, "trace"),
            number_field(&saves, "answer")
        )
    }
}

fn remove_file_if_exists(path: &Path) {
    let _ = fs::remove_file(path);
}

fn env_float(name: &str, fallback: f64) -> f64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(fallback)
}

fn idle_disabled() -> bool {
    env::var("COGNEE_IDLE_DISABLED")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}
