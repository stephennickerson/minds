use crate::arguments::{
    insert_optional_string, insert_optional_strings, optional_bool, optional_string,
    optional_strings, optional_u64, optional_value, required_string,
};
use crate::client::CogneeClient;
use crate::markdown::{json_block, packet, table, text_block, truncate, value_text};
use crate::read_model::{
    LocalForget, ReadDataItem, ReadDataset, ReadEdge, ReadGraph, ReadModel, ReadModelSummary,
    ReadNode, RecallEvidence, RecallRead, RecallRelationship,
};
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
        "sync_read_model" => sync_read_model_packet(client, &arguments),
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
    let summary = ReadModel::from_client(client).summary()?;
    let datasets = ReadModel::from_client(client).datasets(&json!({}))?;
    Ok(packet(
        "Cognee Memory MCP",
        "Agent-visible memory actions are served by the Rust MCP read model.",
        &describe_evidence(&summary, &datasets),
        describe_next(),
        &describe_coverage(client),
    ))
}

fn status_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let summary = ReadModel::from_client(client).summary()?;
    let datasets = ReadModel::from_client(client).datasets(arguments)?;
    Ok(packet(
        "Cognee Rust Read Model Status",
        "Rust read-model status was retrieved locally without calling Python.",
        &status_evidence(arguments, &summary, &datasets),
        status_next(),
        &status_coverage(client),
    ))
}

fn dataset_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let view = optional_string(arguments, "view").unwrap_or_else(|| "summary".to_string());
    match view.as_str() {
        "data" => dataset_data_packet(client, arguments),
        "status" => status_packet(client, arguments),
        "schema" => dataset_schema_packet(client, arguments),
        "raw" => raw_data_packet(client, arguments),
        _ => dataset_summary_packet(client, arguments),
    }
}

fn graph_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let graph = ReadModel::from_client(client).graph(arguments)?;
    Ok(packet(
        "Cognee Rust Graph",
        &graph_answer(&graph),
        &graph_evidence(&graph, arguments),
        graph_next(),
        &graph_coverage(client),
    ))
}

fn search_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let read = ReadModel::from_client(client).recall(arguments)?;
    Ok(packet(
        "Cognee Rust Search",
        &search_answer(&read),
        &search_evidence(&read),
        search_next(),
        &format!(
            "Read model: `{}`. Default search skipped `POST /api/v1/search`.",
            client.settings().read_model_path.display()
        ),
    ))
}

fn recall_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    if recall_wants_presummary(arguments) {
        return recall_presummary_packet(client, arguments);
    }
    recall_read_packet(client, arguments)
}

fn recall_read_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let read = ReadModel::from_client(client).recall(arguments)?;
    Ok(packet(
        "Cognee Recall",
        &rust_recall_answer(&read),
        &rust_recall_evidence(&read),
        recall_next(),
        &rust_recall_coverage(client, &read),
    ))
}

fn recall_presummary_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    require_operator_tools(client)?;
    let read = ReadModel::from_client(client).recall(arguments)?;
    let body = recall_body(client, arguments)?;
    let results = client.post_json("/api/v1/recall", &body)?;
    Ok(packet(
        "Cognee Recall",
        &recall_presummary_answer(&results),
        &recall_presummary_evidence(&body, &results, &read),
        recall_next(),
        "Endpoints: Rust read model first, then `POST /api/v1/recall` because `llm_presummary=true`.",
    ))
}

fn recall_wants_presummary(arguments: &Value) -> bool {
    optional_bool(arguments, "llm_presummary", false)
}

fn add_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    require_operator_tools(client)?;
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
    require_operator_tools(client)?;
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
    require_operator_tools(client)?;
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
    let result = ReadModel::from_client(client).forget(arguments)?;
    Ok(packet(
        "Cognee Rust Forget",
        "The Rust read model applied the targeted local memory invalidation.",
        &forget_evidence(arguments, &result),
        forget_next(),
        &format!(
            "Read model: `{}`. Default forget skipped `POST /api/v1/forget`.",
            client.settings().read_model_path.display()
        ),
    ))
}

fn sync_read_model_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let datasets = ReadModel::from_client(client).sync(client, arguments)?;
    Ok(packet(
        "Cognee Read Model Sync",
        "The Rust read model was refreshed from Cognee backend exports.",
        &sync_evidence(&datasets),
        "Call `recall`, `search`, `inspect_dataset`, or `inspect_graph` for the fast Rust agent surface.",
        "Operator endpoint bridge: datasets/data/graph/raw exports. This tool is hidden unless operator tools are enabled.",
    ))
}

fn schema_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    require_operator_tools(client)?;
    let dataset = dataset_handle(client, arguments)?;
    let schema = client.get_json(&format!("/api/v1/datasets/{}/schema", dataset.id))?;
    Ok(packet(
        "Cognee Schema",
        "Dataset schema was retrieved.",
        &format!(
            "Dataset: `{}` / `{}`\n\n{}",
            dataset.name,
            dataset.id,
            json_block(&schema)
        ),
        schema_next(),
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

fn dataset_summary_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let datasets = ReadModel::from_client(client).datasets(arguments)?;
    Ok(packet(
        "Cognee Rust Datasets",
        &dataset_summary_answer(&datasets),
        &dataset_summary_evidence(&datasets),
        dataset_summary_next(),
        &format!(
            "Read model: `{}`. Default dataset inspection skipped `GET /api/v1/datasets`.",
            client.settings().read_model_path.display()
        ),
    ))
}

fn dataset_data_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let data = ReadModel::from_client(client).data_items(arguments)?;
    Ok(packet(
        "Cognee Rust Dataset Data",
        &data_answer(&data),
        &data_evidence(&data),
        data_next(),
        &data_coverage(client),
    ))
}

fn dataset_schema_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let datasets = ReadModel::from_client(client).datasets(arguments)?;
    Ok(packet(
        "Cognee Rust Dataset Schema",
        "No Python schema endpoint was called; the Rust read model exposes its local table shape.",
        &schema_evidence(&datasets),
        schema_next(),
        &schema_coverage(client),
    ))
}

fn raw_data_packet(client: &CogneeClient, arguments: &Value) -> Result<String> {
    let data_id = required_string(arguments, "data_id")?;
    let item = ReadModel::from_client(client)
        .raw_data_item(arguments)?
        .ok_or_else(|| anyhow!("data item not found in Rust read model: {data_id}"))?;
    Ok(packet(
        "Cognee Rust Raw Data",
        "Raw data was retrieved from the Rust read model.",
        &text_block(&item.raw_text),
        data_next(),
        &raw_coverage(client, &data_id),
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

fn recall_body(client: &CogneeClient, arguments: &Value) -> Result<Value> {
    let mut body = base_query_body(client, arguments)?;
    insert_search_type(&mut body, arguments, "GRAPH_COMPLETION");
    insert_optional_value(&mut body, "scope", optional_value(arguments, "scope"));
    body.insert("onlyContext".to_string(), json!(false));
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

fn describe_evidence(summary: &ReadModelSummary, datasets: &[ReadDataset]) -> String {
    format!(
        "{}\n\n{}",
        read_model_summary_table(summary),
        dataset_summary_evidence(datasets)
    )
}

fn describe_next() -> &'static str {
    "Call `recall` for ranked evidence, `search` for direct read-model lookup, or `inspect_dataset` to choose a dataset handle."
}

fn describe_coverage(client: &CogneeClient) -> String {
    format!(
        "Read model: `{}`. Agent default surface performs local Rust reads and does not call Python/Cognee HTTP endpoints.",
        client.settings().read_model_path.display()
    )
}

fn status_evidence(
    arguments: &Value,
    summary: &ReadModelSummary,
    datasets: &[ReadDataset],
) -> String {
    format!(
        "Requested handle:\n\n{}\n\nRead model counts:\n\n{}\n\nDatasets:\n\n{}",
        json_block(arguments),
        read_model_summary_table(summary),
        dataset_summary_evidence(datasets)
    )
}

fn status_next() -> &'static str {
    "Call `inspect_dataset` with `view: data` for local contents, or `sync_read_model` in operator mode after background ingestion finishes."
}

fn status_coverage(client: &CogneeClient) -> String {
    format!(
        "Read model: `{}`. Default status skipped Python health/status endpoints.",
        client.settings().read_model_path.display()
    )
}

fn read_model_summary_table(summary: &ReadModelSummary) -> String {
    table(
        &["Datasets", "Data items", "Nodes", "Edges"],
        &[vec![
            summary.dataset_count.to_string(),
            summary.data_item_count.to_string(),
            summary.node_count.to_string(),
            summary.edge_count.to_string(),
        ]],
    )
}

fn dataset_summary_answer(datasets: &[ReadDataset]) -> String {
    format!("Rust read model contains {} dataset(s).", datasets.len())
}

fn dataset_summary_evidence(datasets: &[ReadDataset]) -> String {
    table(&["Name", "Dataset ID", "Created"], &dataset_rows(datasets))
}

fn dataset_rows(datasets: &[ReadDataset]) -> Vec<Vec<String>> {
    datasets
        .iter()
        .map(|dataset| {
            vec![
                dataset.name.clone(),
                dataset.id.clone(),
                "local".to_string(),
            ]
        })
        .collect()
}

fn dataset_summary_next() -> &'static str {
    "Call `inspect_dataset` with `dataset_name` and `view: data`, or call `search` with `datasets: [name]`."
}

fn data_answer(data: &[ReadDataItem]) -> String {
    format!("Rust read model returned {} data item(s).", data.len())
}

fn data_evidence(data: &[ReadDataItem]) -> String {
    table(&["Dataset ID", "Name", "Data ID", "Mime"], &data_rows(data))
}

fn data_rows(data: &[ReadDataItem]) -> Vec<Vec<String>> {
    data.iter()
        .map(|item| {
            vec![
                item.dataset_id.clone(),
                item.name.clone(),
                item.data_id.clone(),
                item.mime.clone(),
            ]
        })
        .collect()
}

fn data_next() -> &'static str {
    "Call `search` with the dataset handle, or `inspect_graph` with the same `dataset_id`."
}

fn data_coverage(client: &CogneeClient) -> String {
    format!(
        "Read model: `{}`. Default data inspection skipped Python dataset/data endpoints.",
        client.settings().read_model_path.display()
    )
}

fn graph_answer(graph: &ReadGraph) -> String {
    format!(
        "Graph contains {} node(s) and {} edge(s).",
        graph.nodes.len(),
        graph.edges.len()
    )
}

fn graph_evidence(graph: &ReadGraph, arguments: &Value) -> String {
    let limit = optional_u64(arguments, "limit", 25) as usize;
    format!(
        "Datasets:\n\n{}\n\nNodes:\n\n{}\n\nEdges:\n\n{}",
        dataset_summary_evidence(&graph.datasets),
        table(
            &["Label", "Type", "Node ID", "Text"],
            &node_rows(&graph.nodes, limit)
        ),
        table(
            &["Label", "Source", "Target", "Handle"],
            &edge_rows(&graph.edges, limit)
        )
    )
}

fn node_rows(nodes: &[ReadNode], limit: usize) -> Vec<Vec<String>> {
    nodes
        .iter()
        .take(limit)
        .map(|node| {
            vec![
                node.label.clone(),
                node.node_type.clone(),
                node.id.clone(),
                truncate(&node.body, 160),
            ]
        })
        .collect()
}

fn edge_rows(edges: &[ReadEdge], limit: usize) -> Vec<Vec<String>> {
    edges
        .iter()
        .take(limit)
        .map(|edge| {
            vec![
                edge.label.clone(),
                edge.source_label.clone(),
                edge.target_label.clone(),
                format!("edge: {} -> {}", edge.source_id, edge.target_id),
            ]
        })
        .collect()
}

fn graph_next() -> &'static str {
    "Call `recall` or `search` with a query to retrieve ranked evidence from this graph."
}

fn graph_coverage(client: &CogneeClient) -> String {
    format!(
        "Read model: `{}`. Default graph inspection skipped Python graph endpoints.",
        client.settings().read_model_path.display()
    )
}

fn search_answer(read: &RecallRead) -> String {
    format!(
        "Rust search returned {} ranked evidence item(s) for `{}`.",
        read.evidence.len(),
        read.query
    )
}

fn search_evidence(read: &RecallRead) -> String {
    format!(
        "Datasets:\n\n{}\n\nRanked evidence:\n\n{}",
        recall_dataset_table(read),
        recall_evidence_table(&read.evidence)
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
    "Call `recall` with the same query when you need ranked evidence, or `inspect_graph` for graph structure."
}

fn rust_recall_answer(read: &RecallRead) -> String {
    format!(
        "Rust recall returned {} ranked evidence item(s) and {} graph relationship(s) for `{}` without LLM presummary.",
        read.evidence.len(),
        read.relationships.len(),
        read.query
    )
}

fn rust_recall_evidence(read: &RecallRead) -> String {
    format!(
        "Datasets:\n\n{}\n\nRanked evidence:\n\n{}\n\nGraph relationships:\n\n{}\n\nCoverage notes:\n\n{}",
        recall_dataset_table(read),
        recall_evidence_table(&read.evidence),
        recall_relationship_table(&read.relationships),
        recall_coverage_notes(read, false)
    )
}

fn recall_dataset_table(read: &RecallRead) -> String {
    table(
        &["Dataset", "Dataset ID"],
        &read
            .datasets
            .iter()
            .map(|dataset| vec![dataset.name.clone(), dataset.id.clone()])
            .collect::<Vec<_>>(),
    )
}

fn recall_evidence_table(evidence: &[RecallEvidence]) -> String {
    table(
        &["Rank", "Score", "Kind", "Label", "Handle", "Evidence"],
        &evidence.iter().map(recall_evidence_row).collect::<Vec<_>>(),
    )
}

fn recall_evidence_row(item: &RecallEvidence) -> Vec<String> {
    vec![
        item.rank.to_string(),
        item.score.to_string(),
        item.source_kind.clone(),
        item.label.clone(),
        item.handle.clone(),
        item.text.clone(),
    ]
}

fn recall_relationship_table(relationships: &[RecallRelationship]) -> String {
    table(
        &[
            "Rank",
            "Score",
            "Source",
            "Relationship",
            "Target",
            "Handle",
        ],
        &relationships
            .iter()
            .map(recall_relationship_row)
            .collect::<Vec<_>>(),
    )
}

fn recall_relationship_row(item: &RecallRelationship) -> Vec<String> {
    vec![
        item.rank.to_string(),
        item.score.to_string(),
        item.source.clone(),
        item.relationship.clone(),
        item.target.clone(),
        item.handle.clone(),
    ]
}

fn recall_coverage_notes(read: &RecallRead, llm_presummary: bool) -> String {
    format!(
        "`llm_presummary={}`; `search_type={}`; synced dataset(s): {}; source handles include `dataset_id`, `data_id`, `node_id`, and graph `edge`.\n\nConfidence: {}",
        llm_presummary,
        read.search_type,
        read.synced_dataset_count,
        recall_confidence(read)
    )
}

fn rust_recall_coverage(client: &CogneeClient, read: &RecallRead) -> String {
    format!(
        "Read model: `{}`. Default recall skipped `POST /api/v1/recall`; source API sync uses datasets/data/graph/raw exports only. Query: `{}`. Confidence: {}",
        client.settings().read_model_path.display(),
        read.query,
        recall_confidence(read)
    )
}

fn recall_confidence(read: &RecallRead) -> String {
    let top_score = read.evidence.first().map(|item| item.score).unwrap_or(0);
    match (read.evidence.len(), read.relationships.len(), top_score) {
        (evidence, relationships, score) if evidence >= 3 && relationships > 0 && score >= 30 => {
            "high; multiple ranked evidence items and graph relationships matched the query."
                .to_string()
        }
        (evidence, relationships, score) if evidence > 0 && relationships > 0 && score >= 10 => {
            "medium; evidence and graph relationships matched, but the packet should be synthesized by the calling agent.".to_string()
        }
        (evidence, _, _) if evidence > 0 => {
            "low; lexical evidence matched, but graph relationship support is thin.".to_string()
        }
        _ => "low; no ranked evidence matched the read model.".to_string(),
    }
}

fn recall_presummary_answer(results: &Value) -> String {
    format!(
        "Cognee returned {} presummary item(s) after Rust evidence retrieval.",
        results.as_array().map(Vec::len).unwrap_or(0)
    )
}

fn recall_presummary_evidence(body: &Value, results: &Value, read: &RecallRead) -> String {
    format!(
        "Presummary request:\n\n{}\n\nPresummary:\n\n{}\n\nRanked evidence used first:\n\n{}\n\nGraph relationships:\n\n{}\n\nCoverage notes:\n\n{}",
        json_block(body),
        result_rows(results),
        recall_evidence_table(&read.evidence),
        recall_relationship_table(&read.relationships),
        recall_coverage_notes(read, true)
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

fn forget_evidence(arguments: &Value, result: &LocalForget) -> String {
    format!(
        "Forget request:\n\n{}\n\nRows removed from Rust read model:\n\n{}",
        json_block(arguments),
        table(
            &["Datasets", "Data items", "Nodes", "Edges"],
            &[vec![
                result.deleted_datasets.to_string(),
                result.deleted_data_items.to_string(),
                result.deleted_nodes.to_string(),
                result.deleted_edges.to_string(),
            ]]
        )
    )
}

fn forget_next() -> &'static str {
    "Call `search` or `recall` to confirm the obsolete memory no longer appears."
}

fn sync_evidence(datasets: &[ReadDataset]) -> String {
    dataset_summary_evidence(datasets)
}

fn schema_evidence(datasets: &[ReadDataset]) -> String {
    format!(
        "Datasets:\n\n{}\n\nLocal schema:\n\n{}",
        dataset_summary_evidence(datasets),
        table(
            &["Table", "Purpose"],
            &[
                vec!["datasets".to_string(), "stable dataset handles".to_string()],
                vec![
                    "data_items".to_string(),
                    "raw source text and data handles".to_string()
                ],
                vec![
                    "nodes".to_string(),
                    "graph nodes and node bodies".to_string()
                ],
                vec![
                    "edges".to_string(),
                    "graph relationships/triples".to_string()
                ],
            ]
        )
    )
}

fn schema_next() -> &'static str {
    "Call `inspect_dataset` with `view: data`, or `inspect_graph` for nodes and relationships."
}

fn schema_coverage(client: &CogneeClient) -> String {
    format!(
        "Read model: `{}`. Default schema inspection skipped Python schema endpoints.",
        client.settings().read_model_path.display()
    )
}

fn raw_coverage(client: &CogneeClient, data_id: &str) -> String {
    format!(
        "Read model: `{}`. Data ID: `{}`. Default raw inspection skipped Python raw-data endpoints.",
        client.settings().read_model_path.display(),
        data_id
    )
}

fn default_tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "describe",
            "Return a Rust read-model Markdown packet orienting the agent to Cognee memory.",
            empty_schema(),
        ),
        tool(
            "get_status",
            "Return a local Rust read-model status packet.",
            dataset_filter_schema(),
        ),
        tool(
            "recall",
            "Return a Rust read-model packet with ranked evidence, handles, graph relationships, and confidence notes.",
            query_schema(),
        ),
        tool(
            "search",
            "Return a Rust read-model packet with direct ranked evidence.",
            query_schema(),
        ),
        tool(
            "inspect_dataset",
            "Return a Rust read-model packet for dataset summary, data, schema, or raw data.",
            dataset_schema(),
        ),
        tool(
            "inspect_graph",
            "Return a Rust read-model packet with readable graph nodes and edges.",
            graph_schema(),
        ),
        tool(
            "remember",
            "Submit explicit agent memory through the Rust MCP bridge.",
            upload_schema(),
        ),
        tool(
            "forget",
            "Return a Rust read-model packet after targeted local memory invalidation.",
            forget_schema(),
        ),
        tool(
            "sync_read_model",
            "Refresh the Rust read model from Cognee backend exports after ingestion.",
            dataset_filter_schema(),
        ),
    ]
}

fn operator_tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "add",
            "Operator/background bridge after staging data in Cognee.",
            upload_schema(),
        ),
        tool(
            "cognify",
            "Operator/background bridge after starting Cognee graph/vector construction.",
            cognify_schema(),
        ),
        tool(
            "improve",
            "Operator/background bridge after starting Cognee improvement/enrichment.",
            improve_schema(),
        ),
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

fn dataset_filter_schema() -> Value {
    json!({ "type": "object", "properties": { "dataset_name": { "type": "string" }, "dataset_id": { "type": "string" }, "datasets": { "type": "array", "items": { "type": "string" } }, "dataset_ids": { "type": "array", "items": { "type": "string" } } } })
}

fn upload_schema() -> Value {
    json!({ "type": "object", "properties": { "data": { "type": "array", "items": { "type": "string" } }, "dataset_name": { "type": "string" }, "dataset_id": { "type": "string" }, "run_in_background": { "type": "boolean" }, "node_set": { "type": "array", "items": { "type": "string" } } }, "required": ["data"] })
}

fn query_schema() -> Value {
    json!({ "type": "object", "properties": { "query": { "type": "string" }, "datasets": { "type": "array", "items": { "type": "string" } }, "dataset_ids": { "type": "array", "items": { "type": "string" } }, "search_type": { "type": "string" }, "top_k": { "type": "integer" }, "only_context": { "type": "boolean" }, "llm_presummary": { "type": "boolean" } }, "required": ["query"] })
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

    #[test]
    fn recall_presummary_defaults_to_fast_mode() {
        assert!(!recall_wants_presummary(&json!({})));
        assert!(recall_wants_presummary(&json!({ "llm_presummary": true })));
    }

    #[test]
    fn recall_schema_exposes_presummary_switch() {
        assert!(query_schema().to_string().contains("llm_presummary"));
    }
}
