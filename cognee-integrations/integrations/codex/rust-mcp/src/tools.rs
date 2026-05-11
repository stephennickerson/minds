use crate::arguments::{
    insert_optional_string, insert_optional_strings, optional_bool, optional_string,
    optional_strings, optional_u64, optional_value, required_string,
};
use crate::client::CogneeClient;
use crate::markdown::{json_block, packet, table, text_block, truncate, value_count, value_text};
use crate::uploads::{
    append_bool_field, append_string_list_field, append_text_field, form_with_uploads,
};
use anyhow::{Result, anyhow};
use serde_json::{Map, Value, json};

pub(crate) fn tool_definitions(operator_tools_enabled: bool) -> Vec<Value> {
    let mut tools = default_tool_definitions();
    if operator_tools_enabled {
        tools.extend(operator_tool_definitions());
    }
    tools
}

pub(crate) fn call_tool(client: &CogneeClient, name: &str, arguments: Value) -> Result<String> {
    match name {
        "describe" => describe_packet(client),
        "get_status" => status_packet(client, &arguments),
        "inspect_dataset" => dataset_packet(client, &arguments),
        "inspect_graph" => graph_packet(client, &arguments),
        "search" => search_packet(client, &arguments),
        "recall" => recall_packet(client, &arguments),
        "add" => add_packet(client, &arguments),
        "remember" => remember_packet(client, &arguments),
        "cognify" => cognify_packet(client, &arguments),
        "improve" => improve_packet(client, &arguments),
        "forget" => forget_packet(client, &arguments),
        "manage_schema" => schema_packet(client, &arguments),
        "manage_ontology" => ontology_packet(client, &arguments),
        _ => Err(anyhow!("unknown tool: {name}")),
    }
}

#[cfg(test)]
fn error_packet(title: &str, detail: &str) -> String {
    packet(
        title,
        "The operation failed or was refused.",
        &text_block(detail),
        "Call `describe` to verify the MCP surface, or `get_status` to verify Cognee health.",
        "The error was produced by the Rust MCP before or during a Cognee API call.",
    )
}

fn describe_packet(client: &CogneeClient) -> Result<String> {
    let health = client.get_json("/health")?;
    let datasets = client
        .get_json("/api/v1/datasets")
        .unwrap_or_else(|error| json!({ "error": error.to_string() }));
    Ok(packet(
        "Cognee Memory MCP",
        &describe_answer(client),
        &describe_evidence(&health, &datasets),
        describe_next(),
        &describe_coverage(client),
    ))
}

fn status_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let health = client.get_json("/health")?;
    let detailed = detailed_health(client, arguments);
    let status = client.get_json(&status_path(client, arguments))?;
    Ok(packet(
        "Cognee Status",
        "Cognee status was retrieved from the live backend.",
        &status_evidence(arguments, &health, &detailed, &status),
        status_next(),
        status_coverage(arguments),
    ))
}

fn status_path(client: &CogneeClient, arguments: &Value) -> String {
    status_dataset_id(client, arguments)
        .map(|dataset_id| format!("/api/v1/datasets/status?dataset={dataset_id}"))
        .unwrap_or_else(|| "/api/v1/datasets/status".to_string())
}

fn status_dataset_id(client: &CogneeClient, arguments: &Value) -> Option<String> {
    optional_string(arguments, "dataset_id").or_else(|| {
        status_dataset_name(client, arguments).and_then(|name| {
            dataset_handle_by_name(client, &name)
                .ok()
                .map(|dataset| dataset.id)
        })
    })
}

fn status_dataset_name(_client: &CogneeClient, arguments: &Value) -> Option<String> {
    optional_string(arguments, "dataset_name")
}

fn dataset_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let view = optional_string(arguments, "view").unwrap_or_else(|| "summary".to_string());
    match view.as_str() {
        "data" => dataset_data_packet(client, arguments),
        "status" => status_packet(client, arguments),
        "schema" => dataset_schema_packet(client, arguments),
        "raw" => raw_data_packet(client, arguments),
        _ => dataset_summary_packet(client),
    }
}

fn graph_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let dataset = dataset_handle(client, arguments)?;
    let graph = client.get_json(&format!("/api/v1/datasets/{}/graph", dataset.id))?;
    Ok(packet(
        "Cognee Graph",
        &graph_answer(&graph),
        &graph_evidence(&dataset, &graph, arguments),
        &graph_next(&dataset),
        &graph_coverage(&dataset),
    ))
}

fn search_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let body = search_body(client, arguments)?;
    let results = client.post_json("/api/v1/search", &body)?;
    Ok(packet(
        "Cognee Search",
        &search_answer(&results),
        &search_evidence(&body, &results),
        search_next(),
        "Endpoint: `POST /api/v1/search`.",
    ))
}

fn recall_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let body = recall_body(client, arguments)?;
    let results = client.post_json("/api/v1/recall", &body)?;
    Ok(packet(
        "Cognee Recall",
        &recall_answer(&results),
        &recall_evidence(&body, &results),
        recall_next(),
        "Endpoint: `POST /api/v1/recall`.",
    ))
}

fn add_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let values = upload_values(arguments)?;
    let form = upload_form(client, arguments, &values)?;
    let response = client.post_multipart("/api/v1/add", form)?;
    Ok(packet(
        "Cognee Add",
        "Data was submitted to Cognee for staging.",
        &operation_evidence(arguments, &response),
        operation_next(),
        "Endpoint: `POST /api/v1/add`.",
    ))
}

fn remember_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let values = upload_values(arguments)?;
    let form = remember_form(client, arguments, &values)?;
    let response = client.post_multipart("/api/v1/remember", form)?;
    Ok(packet(
        "Cognee Remember",
        "Data was submitted to Cognee for memory ingestion.",
        &operation_evidence(arguments, &response),
        operation_next(),
        "Endpoint: `POST /api/v1/remember`.",
    ))
}

fn cognify_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let body = cognify_body(arguments);
    let response = client.post_json("/api/v1/cognify", &body)?;
    Ok(packet(
        "Cognee Cognify",
        "Cognee accepted the cognify request.",
        &operation_evidence(&body, &response),
        "Call `get_status` with the dataset handle or pipeline handle returned above.",
        "Endpoint: `POST /api/v1/cognify`.",
    ))
}

fn improve_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let body = improve_body(arguments);
    let response = client.post_json("/api/v1/improve", &body)?;
    Ok(packet(
        "Cognee Improve",
        "Cognee accepted the improve request.",
        &operation_evidence(&body, &response),
        "Call `get_status` with the dataset handle or pipeline handle returned above.",
        "Endpoint: `POST /api/v1/improve`.",
    ))
}

fn forget_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    reject_broad_forget(client, arguments)?;
    let body = forget_body(arguments);
    let response = client.post_json("/api/v1/forget", &body)?;
    Ok(packet(
        "Cognee Forget",
        "Cognee accepted the targeted memory invalidation request.",
        &forget_evidence(arguments, &response),
        forget_next(),
        "Endpoint: `POST /api/v1/forget`.",
    ))
}

fn schema_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    require_operator_tools(client)?;
    let dataset = dataset_handle(client, arguments)?;
    let schema = client.get_json(&format!("/api/v1/datasets/{}/schema", dataset.id))?;
    Ok(packet(
        "Cognee Schema",
        "Dataset schema was retrieved.",
        &schema_evidence(&dataset, &schema),
        &schema_next(&dataset),
        "Endpoint: `GET /api/v1/datasets/{dataset_id}/schema`.",
    ))
}

fn ontology_packet(client: &CogneeClient, _arguments: &Value) -> Result<String> {
    require_operator_tools(client)?;
    let ontologies = client.get_json("/api/v1/ontologies")?;
    Ok(packet(
        "Cognee Ontologies",
        "Ontology records were retrieved.",
        &json_block(&ontologies),
        "Call `describe` to return to the default memory surface.",
        "Endpoint: `GET /api/v1/ontologies`.",
    ))
}

#[derive(Clone)]
struct DatasetHandle {
    id: String,
    name: String,
}

fn dataset_summary_packet(client: &CogneeClient) -> Result<String> {
    let datasets = client.get_json("/api/v1/datasets")?;
    Ok(packet(
        "Cognee Datasets",
        &dataset_summary_answer(&datasets),
        &dataset_summary_evidence(&datasets),
        dataset_summary_next(),
        "Endpoint: `GET /api/v1/datasets`.",
    ))
}

fn dataset_data_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let dataset = dataset_handle(client, arguments)?;
    let data = client.get_json(&format!("/api/v1/datasets/{}/data", dataset.id))?;
    Ok(packet(
        "Cognee Dataset Data",
        &data_answer(&data),
        &data_evidence(&dataset, &data),
        &data_next(&dataset),
        &data_coverage(&dataset),
    ))
}

fn dataset_schema_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let dataset = dataset_handle(client, arguments)?;
    let schema = client.get_json(&format!("/api/v1/datasets/{}/schema", dataset.id))?;
    Ok(packet(
        "Cognee Dataset Schema",
        "Dataset schema was retrieved for inspection.",
        &schema_evidence(&dataset, &schema),
        &schema_next(&dataset),
        &schema_coverage(&dataset),
    ))
}

fn raw_data_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let dataset = dataset_handle(client, arguments)?;
    let data_id = required_string(arguments, "data_id")?;
    let raw = client.get_text(&format!(
        "/api/v1/datasets/{}/data/{}/raw",
        dataset.id, data_id
    ))?;
    Ok(packet(
        "Cognee Raw Data",
        "Raw data was retrieved without writing a file.",
        &text_block(&raw),
        &data_next(&dataset),
        &raw_coverage(&dataset, &data_id),
    ))
}

fn dataset_handle(client: &CogneeClient, arguments: &Value) -> Result<DatasetHandle> {
    if let Some(id) = optional_string(arguments, "dataset_id") {
        return Ok(DatasetHandle {
            id,
            name: optional_string(arguments, "dataset_name").unwrap_or_default(),
        });
    }
    let name = required_string(arguments, "dataset_name")?;
    dataset_handle_by_name(client, &name)
}

fn dataset_handle_by_name(client: &CogneeClient, name: &str) -> Result<DatasetHandle> {
    let datasets = client.get_json("/api/v1/datasets")?;
    dataset_from_list(&datasets, name).ok_or_else(|| anyhow!("dataset not found: {name}"))
}

fn dataset_from_list(datasets: &Value, name: &str) -> Option<DatasetHandle> {
    datasets
        .as_array()?
        .iter()
        .find_map(|dataset| matching_dataset(dataset, name))
}

fn matching_dataset(dataset: &Value, name: &str) -> Option<DatasetHandle> {
    (value_text(dataset, "name") == name).then(|| DatasetHandle {
        id: value_text(dataset, "id"),
        name: name.to_string(),
    })
}

fn search_body(client: &CogneeClient, arguments: &Value) -> Result<Value> {
    let mut body = base_query_body(client, arguments)?;
    insert_search_type(&mut body, arguments, "CHUNKS");
    Ok(Value::Object(body))
}

fn recall_body(client: &CogneeClient, arguments: &Value) -> Result<Value> {
    let mut body = base_query_body(client, arguments)?;
    insert_search_type(&mut body, arguments, "GRAPH_COMPLETION");
    insert_optional_value(&mut body, "scope", optional_value(arguments, "scope"));
    Ok(Value::Object(body))
}

fn base_query_body(client: &CogneeClient, arguments: &Value) -> Result<Map<String, Value>> {
    let mut body = Map::new();
    body.insert(
        "query".to_string(),
        json!(required_string(arguments, "query")?),
    );
    body.insert(
        "topK".to_string(),
        json!(optional_u64(arguments, "top_k", client.settings().top_k)),
    );
    body.insert(
        "onlyContext".to_string(),
        json!(optional_bool(arguments, "only_context", true)),
    );
    body.insert(
        "verbose".to_string(),
        json!(optional_bool(arguments, "verbose", false)),
    );
    insert_optional_strings(
        &mut body,
        "datasets",
        optional_strings(arguments, "datasets"),
    );
    insert_optional_strings(
        &mut body,
        "datasetIds",
        optional_strings(arguments, "dataset_ids"),
    );
    insert_optional_strings(
        &mut body,
        "nodeName",
        optional_strings(arguments, "node_name"),
    );
    insert_optional_string(
        &mut body,
        "systemPrompt",
        optional_string(arguments, "system_prompt"),
    );
    Ok(body)
}

fn insert_search_type(body: &mut Map<String, Value>, arguments: &Value, fallback: &str) {
    let search_type =
        optional_string(arguments, "search_type").unwrap_or_else(|| fallback.to_string());
    body.insert("searchType".to_string(), json!(search_type));
}

fn insert_optional_value(body: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        body.insert(key.to_string(), value);
    }
}

fn upload_values(arguments: &Value) -> Result<Vec<String>> {
    optional_strings(arguments, "data").ok_or_else(|| anyhow!("missing upload data"))
}

fn upload_form(
    client: &CogneeClient,
    arguments: &Value,
    values: &[String],
) -> Result<reqwest::blocking::multipart::Form> {
    let form = form_with_uploads(values, client.settings().max_upload_bytes)?;
    let form = append_text_field(
        form,
        "datasetName",
        optional_string(arguments, "dataset_name"),
    );
    let form = append_text_field(form, "datasetId", optional_string(arguments, "dataset_id"));
    let form = append_bool_field(
        form,
        "run_in_background",
        optional_bool(arguments, "run_in_background", false),
    );
    Ok(append_string_list_field(
        form,
        "node_set",
        &optional_strings(arguments, "node_set").unwrap_or_default(),
    ))
}

fn remember_form(
    client: &CogneeClient,
    arguments: &Value,
    values: &[String],
) -> Result<reqwest::blocking::multipart::Form> {
    let form = upload_form(client, arguments, values)?;
    let form = append_text_field(
        form,
        "custom_prompt",
        optional_string(arguments, "custom_prompt"),
    );
    Ok(append_text_field(
        form,
        "chunks_per_batch",
        Some(optional_u64(arguments, "chunks_per_batch", 10).to_string()),
    ))
}

fn cognify_body(arguments: &Value) -> Value {
    let mut body = Map::new();
    insert_optional_strings(
        &mut body,
        "datasets",
        optional_strings(arguments, "datasets"),
    );
    insert_optional_strings(
        &mut body,
        "datasetIds",
        optional_strings(arguments, "dataset_ids"),
    );
    insert_optional_value(
        &mut body,
        "graphModel",
        optional_value(arguments, "graph_model"),
    );
    insert_optional_string(
        &mut body,
        "customPrompt",
        optional_string(arguments, "custom_prompt"),
    );
    insert_optional_strings(
        &mut body,
        "ontologyKey",
        optional_strings(arguments, "ontology_key"),
    );
    body.insert(
        "runInBackground".to_string(),
        json!(optional_bool(arguments, "run_in_background", true)),
    );
    Value::Object(body)
}

fn improve_body(arguments: &Value) -> Value {
    let mut body = Map::new();
    insert_optional_strings(
        &mut body,
        "extractionTasks",
        optional_strings(arguments, "extraction_tasks"),
    );
    insert_optional_strings(
        &mut body,
        "enrichmentTasks",
        optional_strings(arguments, "enrichment_tasks"),
    );
    insert_optional_strings(
        &mut body,
        "nodeName",
        optional_strings(arguments, "node_name"),
    );
    insert_optional_string(
        &mut body,
        "datasetName",
        optional_string(arguments, "dataset_name"),
    );
    insert_optional_string(
        &mut body,
        "datasetId",
        optional_string(arguments, "dataset_id"),
    );
    body.insert(
        "runInBackground".to_string(),
        json!(optional_bool(arguments, "run_in_background", true)),
    );
    Value::Object(body)
}

fn forget_body(arguments: &Value) -> Value {
    let mut body = Map::new();
    insert_optional_string(&mut body, "dataId", optional_string(arguments, "data_id"));
    insert_optional_string(
        &mut body,
        "dataset",
        optional_string(arguments, "dataset_name")
            .or_else(|| optional_string(arguments, "dataset_id")),
    );
    body.insert(
        "everything".to_string(),
        json!(optional_bool(arguments, "everything", false)),
    );
    body.insert(
        "memoryOnly".to_string(),
        json!(optional_bool(arguments, "memory_only", false)),
    );
    Value::Object(body)
}

fn reject_broad_forget(client: &CogneeClient, arguments: &Value) -> Result<()> {
    let broad = optional_bool(arguments, "everything", false)
        || optional_string(arguments, "data_id").is_none();
    if broad && !client.settings().destructive_tools_enabled {
        return Err(anyhow!(
            "broad forget requires COGNEE_MCP_ENABLE_DESTRUCTIVE_TOOLS=true"
        ));
    }
    Ok(())
}

fn require_operator_tools(client: &CogneeClient) -> Result<()> {
    client
        .settings()
        .operator_tools_enabled
        .then_some(())
        .ok_or_else(|| anyhow!("operator tools are disabled"))
}

fn detailed_health(client: &CogneeClient, arguments: &Value) -> Value {
    if optional_bool(arguments, "include_detailed_health", false) {
        return client
            .get_json("/health/detailed")
            .unwrap_or_else(|error| json!({ "error": error.to_string() }));
    }
    json!({ "skipped": true })
}

fn describe_answer(client: &CogneeClient) -> String {
    format!(
        "Cognee is available through `{}` using `{}` auth mode.",
        client.settings().service_url,
        client.settings().auth_mode()
    )
}

fn describe_evidence(health: &Value, datasets: &Value) -> String {
    format!(
        "{}\n\n{}",
        json_block(health),
        dataset_summary_evidence(datasets)
    )
}

fn describe_next() -> &'static str {
    "Call `search` for direct retrieval, `recall` for memory-grounded synthesis, or `inspect_dataset` to choose a dataset handle."
}

fn describe_coverage(client: &CogneeClient) -> String {
    format!(
        "Endpoints: `GET /health`, `GET /api/v1/datasets`. Service URL: `{}`.",
        client.settings().service_url
    )
}

fn status_evidence(arguments: &Value, health: &Value, detailed: &Value, status: &Value) -> String {
    format!(
        "Requested handle:\n\n{}\n\nHealth:\n\n{}\n\nDetailed health:\n\n{}\n\nDataset status:\n\n{}",
        json_block(arguments),
        json_block(health),
        json_block(detailed),
        json_block(status)
    )
}

fn status_next() -> &'static str {
    "Call `inspect_dataset` with `view: data` for dataset contents, or `search` when processing is complete."
}

fn status_coverage(_arguments: &Value) -> &'static str {
    "Endpoints: `GET /health`, optional `GET /health/detailed`, `GET /api/v1/datasets/status`."
}

fn dataset_summary_answer(datasets: &Value) -> String {
    format!(
        "Cognee returned {} dataset(s).",
        datasets.as_array().map(Vec::len).unwrap_or(0)
    )
}

fn dataset_summary_evidence(datasets: &Value) -> String {
    table(&["Name", "Dataset ID", "Created"], &dataset_rows(datasets))
}

fn dataset_rows(datasets: &Value) -> Vec<Vec<String>> {
    datasets
        .as_array()
        .map(|values| dataset_rows_from_array(values))
        .unwrap_or_default()
}

fn dataset_rows_from_array(datasets: &[Value]) -> Vec<Vec<String>> {
    datasets.iter().map(dataset_row).collect()
}

fn dataset_row(dataset: &Value) -> Vec<String> {
    vec![
        value_text(dataset, "name"),
        value_text(dataset, "id"),
        value_text(dataset, "createdAt"),
    ]
}

fn dataset_summary_next() -> &'static str {
    "Call `inspect_dataset` with `dataset_name` and `view: data`, or call `search` with `datasets: [name]`."
}

fn data_answer(data: &Value) -> String {
    format!(
        "Cognee returned {} data item(s).",
        data.as_array().map(Vec::len).unwrap_or(0)
    )
}

fn data_evidence(dataset: &DatasetHandle, data: &Value) -> String {
    format!(
        "Dataset: `{}` / `{}`\n\n{}",
        dataset.name,
        dataset.id,
        table(&["Name", "Data ID", "Mime", "Updated"], &data_rows(data))
    )
}

fn data_rows(data: &Value) -> Vec<Vec<String>> {
    data.as_array()
        .map(|values| data_rows_from_array(values))
        .unwrap_or_default()
}

fn data_rows_from_array(items: &[Value]) -> Vec<Vec<String>> {
    items.iter().map(data_row).collect()
}

fn data_row(item: &Value) -> Vec<String> {
    vec![
        value_text(item, "name"),
        value_text(item, "id"),
        value_text(item, "mimeType"),
        value_text(item, "updatedAt"),
    ]
}

fn data_next(dataset: &DatasetHandle) -> String {
    format!(
        "Call `search` with `datasets: [\"{}\"]`, or `inspect_graph` with `dataset_id: \"{}\"`.",
        dataset.name, dataset.id
    )
}

fn data_coverage(dataset: &DatasetHandle) -> String {
    format!("Endpoint: `GET /api/v1/datasets/{}/data`.", dataset.id)
}

fn graph_answer(graph: &Value) -> String {
    format!(
        "Graph contains {} node(s) and {} edge(s).",
        value_count(graph, "nodes"),
        value_count(graph, "edges")
    )
}

fn graph_evidence(dataset: &DatasetHandle, graph: &Value, arguments: &Value) -> String {
    let limit = optional_u64(arguments, "limit", 25) as usize;
    format!(
        "Dataset: `{}` / `{}`\n\nNodes:\n\n{}\n\nEdges:\n\n{}",
        dataset.name,
        dataset.id,
        table(&["Label", "Type", "Node ID"], &node_rows(graph, limit)),
        table(&["Label", "Source", "Target"], &edge_rows(graph, limit))
    )
}

fn node_rows(graph: &Value, limit: usize) -> Vec<Vec<String>> {
    graph
        .get("nodes")
        .and_then(Value::as_array)
        .map(|nodes| nodes.iter().take(limit).map(node_row).collect())
        .unwrap_or_default()
}

fn node_row(node: &Value) -> Vec<String> {
    vec![
        value_text(node, "label"),
        value_text(node, "type"),
        value_text(node, "id"),
    ]
}

fn edge_rows(graph: &Value, limit: usize) -> Vec<Vec<String>> {
    graph
        .get("edges")
        .and_then(Value::as_array)
        .map(|edges| edges.iter().take(limit).map(edge_row).collect())
        .unwrap_or_default()
}

fn edge_row(edge: &Value) -> Vec<String> {
    vec![
        value_text(edge, "label"),
        value_text(edge, "source"),
        value_text(edge, "target"),
    ]
}

fn graph_next(dataset: &DatasetHandle) -> String {
    format!(
        "Call `search` with `datasets: [\"{}\"]` to retrieve specific memories from this graph.",
        dataset.name
    )
}

fn graph_coverage(dataset: &DatasetHandle) -> String {
    format!("Endpoint: `GET /api/v1/datasets/{}/graph`.", dataset.id)
}

fn search_answer(results: &Value) -> String {
    format!(
        "Cognee returned {} search result(s).",
        results.as_array().map(Vec::len).unwrap_or(0)
    )
}

fn search_evidence(body: &Value, results: &Value) -> String {
    format!(
        "Request:\n\n{}\n\nResults:\n\n{}",
        json_block(body),
        result_rows(results)
    )
}

fn result_rows(results: &Value) -> String {
    results
        .as_array()
        .map(|values| result_table(values))
        .unwrap_or_else(|| json_block(results))
}

fn result_table(results: &[Value]) -> String {
    let rows = results
        .iter()
        .take(10)
        .enumerate()
        .map(result_row)
        .collect::<Vec<_>>();
    table(&["Rank", "Result"], &rows)
}

fn result_row((index, result): (usize, &Value)) -> Vec<String> {
    vec![(index + 1).to_string(), truncate(&result_text(result), 700)]
}

fn result_text(result: &Value) -> String {
    result
        .as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| serde_json::to_string(result).unwrap_or_default())
}

fn search_next() -> &'static str {
    "Call `recall` with the same query when you need answer synthesis, or `inspect_graph` for graph structure."
}

fn recall_answer(results: &Value) -> String {
    format!(
        "Cognee returned {} recall item(s).",
        results.as_array().map(Vec::len).unwrap_or(0)
    )
}

fn recall_evidence(body: &Value, results: &Value) -> String {
    format!(
        "Request:\n\n{}\n\nMemory:\n\n{}",
        json_block(body),
        result_rows(results)
    )
}

fn recall_next() -> &'static str {
    "Call `search` for direct chunks, or `inspect_dataset` to inspect the source data behind these results."
}

fn operation_evidence(request: &Value, response: &Value) -> String {
    format!(
        "Request/arguments:\n\n{}\n\nResponse:\n\n{}",
        json_block(request),
        json_block(response)
    )
}

fn operation_next() -> &'static str {
    "Call `get_status` to watch processing, then `search` or `recall` after processing completes."
}

fn forget_evidence(arguments: &Value, response: &Value) -> String {
    format!(
        "Forget request:\n\n{}\n\nCognee response:\n\n{}",
        json_block(arguments),
        json_block(response)
    )
}

fn forget_next() -> &'static str {
    "Call `search` or `recall` to confirm the obsolete memory no longer appears."
}

fn schema_evidence(dataset: &DatasetHandle, schema: &Value) -> String {
    format!(
        "Dataset: `{}` / `{}`\n\n{}",
        dataset.name,
        dataset.id,
        json_block(schema)
    )
}

fn schema_next(dataset: &DatasetHandle) -> String {
    format!(
        "Call `inspect_dataset` with `dataset_id: \"{}\"` and `view: data` to inspect the data using this schema.",
        dataset.id
    )
}

fn schema_coverage(dataset: &DatasetHandle) -> String {
    format!("Endpoint: `GET /api/v1/datasets/{}/schema`.", dataset.id)
}

fn raw_coverage(dataset: &DatasetHandle, data_id: &str) -> String {
    format!(
        "Endpoint: `GET /api/v1/datasets/{}/data/{}/raw`. No file was written.",
        dataset.id, data_id
    )
}

fn default_tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "describe",
            "Return a Markdown packet orienting the agent to Cognee memory.",
            empty_schema(),
        ),
        tool(
            "get_status",
            "Return a Markdown packet with Cognee health and dataset pipeline status.",
            status_schema(),
        ),
        tool(
            "remember",
            "Return a Markdown packet after one-step memory ingestion.",
            upload_schema(),
        ),
        tool(
            "add",
            "Return a Markdown packet after staging data in a Cognee dataset.",
            upload_schema(),
        ),
        tool(
            "cognify",
            "Return a Markdown packet after starting graph/vector construction.",
            cognify_schema(),
        ),
        tool(
            "recall",
            "Return a Markdown packet with memory-grounded answer/context from Cognee.",
            query_schema(),
        ),
        tool(
            "search",
            "Return a Markdown packet with direct Cognee search results.",
            query_schema(),
        ),
        tool(
            "inspect_dataset",
            "Return a Markdown packet for dataset summary, data, status, schema, or raw data.",
            dataset_schema(),
        ),
        tool(
            "inspect_graph",
            "Return a Markdown packet with readable graph nodes and edges.",
            graph_schema(),
        ),
        tool(
            "improve",
            "Return a Markdown packet after starting Cognee improvement/enrichment.",
            improve_schema(),
        ),
        tool(
            "forget",
            "Return a Markdown packet after targeted memory invalidation.",
            forget_schema(),
        ),
    ]
}

fn operator_tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "manage_schema",
            "Operator tool returning a Markdown packet for dataset schema inspection.",
            schema_tool_schema(),
        ),
        tool(
            "manage_ontology",
            "Operator tool returning a Markdown packet for ontology listing.",
            ontology_tool_schema(),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

fn empty_schema() -> Value {
    json!({ "type": "object", "properties": {} })
}

fn status_schema() -> Value {
    json!({ "type": "object", "properties": { "include_detailed_health": { "type": "boolean" }, "dataset_name": { "type": "string" }, "dataset_id": { "type": "string" } } })
}

fn upload_schema() -> Value {
    json!({ "type": "object", "properties": { "data": { "type": "array", "items": { "type": "string" } }, "dataset_name": { "type": "string" }, "dataset_id": { "type": "string" }, "run_in_background": { "type": "boolean" }, "node_set": { "type": "array", "items": { "type": "string" } } }, "required": ["data"] })
}

fn query_schema() -> Value {
    json!({ "type": "object", "properties": { "query": { "type": "string" }, "datasets": { "type": "array", "items": { "type": "string" } }, "dataset_ids": { "type": "array", "items": { "type": "string" } }, "search_type": { "type": "string" }, "top_k": { "type": "integer" }, "only_context": { "type": "boolean" } }, "required": ["query"] })
}

fn dataset_schema() -> Value {
    json!({ "type": "object", "properties": { "dataset_name": { "type": "string" }, "dataset_id": { "type": "string" }, "view": { "type": "string" }, "data_id": { "type": "string" }, "limit": { "type": "integer" } } })
}

fn graph_schema() -> Value {
    json!({ "type": "object", "properties": { "dataset_name": { "type": "string" }, "dataset_id": { "type": "string" }, "limit": { "type": "integer" } } })
}

fn cognify_schema() -> Value {
    json!({ "type": "object", "properties": { "datasets": { "type": "array", "items": { "type": "string" } }, "dataset_ids": { "type": "array", "items": { "type": "string" } }, "run_in_background": { "type": "boolean" }, "custom_prompt": { "type": "string" } } })
}

fn improve_schema() -> Value {
    json!({ "type": "object", "properties": { "dataset_name": { "type": "string" }, "dataset_id": { "type": "string" }, "run_in_background": { "type": "boolean" }, "node_name": { "type": "array", "items": { "type": "string" } } } })
}

fn forget_schema() -> Value {
    json!({ "type": "object", "properties": { "dataset_name": { "type": "string" }, "dataset_id": { "type": "string" }, "data_id": { "type": "string" }, "memory_only": { "type": "boolean" }, "everything": { "type": "boolean" }, "reason": { "type": "string" } } })
}

fn schema_tool_schema() -> Value {
    json!({ "type": "object", "properties": { "dataset_name": { "type": "string" }, "dataset_id": { "type": "string" } } })
}

fn ontology_tool_schema() -> Value {
    json!({ "type": "object", "properties": {} })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_default_tools_have_schema() {
        for definition in default_tool_definitions() {
            assert!(definition.get("inputSchema").is_some());
        }
    }

    #[test]
    fn error_output_is_markdown_packet() {
        let text = error_packet("Failed", "bad");
        assert!(text.contains("## Navigate Next"));
        assert!(!text.trim_start().starts_with('{'));
    }
}
