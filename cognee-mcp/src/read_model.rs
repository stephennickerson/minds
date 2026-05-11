use crate::arguments::{optional_strings, optional_u64, required_string};
use crate::client::CogneeClient;
use crate::markdown::{truncate, value_text};
use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub(crate) struct ReadDataset {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ReadModelSummary {
    pub(crate) dataset_count: u64,
    pub(crate) data_item_count: u64,
    pub(crate) node_count: u64,
    pub(crate) edge_count: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ReadDataItem {
    pub(crate) dataset_id: String,
    pub(crate) data_id: String,
    pub(crate) name: String,
    pub(crate) mime: String,
    pub(crate) raw_text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ReadNode {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) node_type: String,
    pub(crate) body: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ReadEdge {
    pub(crate) source_id: String,
    pub(crate) target_id: String,
    pub(crate) source_label: String,
    pub(crate) target_label: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ReadGraph {
    pub(crate) datasets: Vec<ReadDataset>,
    pub(crate) nodes: Vec<ReadNode>,
    pub(crate) edges: Vec<ReadEdge>,
}

#[derive(Clone, Debug)]
pub(crate) struct LocalForget {
    pub(crate) deleted_datasets: u64,
    pub(crate) deleted_data_items: u64,
    pub(crate) deleted_nodes: u64,
    pub(crate) deleted_edges: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct RecallEvidence {
    pub(crate) rank: usize,
    pub(crate) score: u64,
    pub(crate) source_kind: String,
    pub(crate) label: String,
    pub(crate) text: String,
    pub(crate) handle: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RecallRead {
    pub(crate) query: String,
    pub(crate) search_type: String,
    pub(crate) datasets: Vec<ReadDataset>,
    pub(crate) evidence: Vec<RecallEvidence>,
    pub(crate) relationships: Vec<RecallRelationship>,
    pub(crate) synced_dataset_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct RecallRelationship {
    pub(crate) rank: usize,
    pub(crate) score: u64,
    pub(crate) source: String,
    pub(crate) relationship: String,
    pub(crate) target: String,
    pub(crate) handle: String,
}

#[derive(Clone, Debug)]
struct EvidenceCandidate {
    source_kind: String,
    label: String,
    text: String,
    handle: String,
}

#[derive(Clone, Debug)]
struct RelationshipCandidate {
    source_id: String,
    target_id: String,
    source: String,
    relationship: String,
    target: String,
}

#[derive(Clone)]
pub(crate) struct ReadModel {
    path: PathBuf,
}

impl ReadModel {
    pub(crate) fn from_client(client: &CogneeClient) -> Self {
        Self {
            path: client.settings().read_model_path.clone(),
        }
    }

    pub(crate) fn summary(&self) -> Result<ReadModelSummary> {
        let connection = self.ready_connection()?;
        Ok(ReadModelSummary {
            dataset_count: count_rows(&connection, "datasets")?,
            data_item_count: count_rows(&connection, "data_items")?,
            node_count: count_rows(&connection, "nodes")?,
            edge_count: count_rows(&connection, "edges")?,
        })
    }

    pub(crate) fn datasets(&self, arguments: &Value) -> Result<Vec<ReadDataset>> {
        let connection = self.ready_connection()?;
        local_target_datasets(&connection, arguments)
    }

    pub(crate) fn data_items(&self, arguments: &Value) -> Result<Vec<ReadDataItem>> {
        let connection = self.ready_connection()?;
        let datasets = local_target_datasets(&connection, arguments)?;
        data_items_for_datasets(&connection, &datasets)
    }

    pub(crate) fn raw_data_item(&self, arguments: &Value) -> Result<Option<ReadDataItem>> {
        let connection = self.ready_connection()?;
        let data_id = required_string(arguments, "data_id")?;
        data_item_by_id(&connection, &data_id)
    }

    pub(crate) fn graph(&self, arguments: &Value) -> Result<ReadGraph> {
        let connection = self.ready_connection()?;
        let datasets = local_target_datasets(&connection, arguments)?;
        Ok(ReadGraph {
            nodes: nodes_for_datasets(&connection, &datasets)?,
            edges: edges_for_datasets(&connection, &datasets)?,
            datasets,
        })
    }

    pub(crate) fn forget(&self, arguments: &Value) -> Result<LocalForget> {
        let mut connection = self.ready_connection()?;
        let transaction = connection.transaction()?;
        let result = if let Some(data_id) = crate::arguments::optional_string(arguments, "data_id")
        {
            forget_data_item(&transaction, &data_id)?
        } else {
            let datasets = local_target_datasets(&transaction, arguments)?;
            forget_datasets(&transaction, &datasets)?
        };
        transaction.commit()?;
        Ok(result)
    }

    pub(crate) fn recall(&self, arguments: &Value) -> Result<RecallRead> {
        let query = required_string(arguments, "query")?;
        let datasets = self.local_datasets(arguments)?;
        let evidence = self.ranked_evidence(arguments, &query, &datasets)?;
        let relationships = self.ranked_relationships(arguments, &query, &datasets)?;
        Ok(RecallRead {
            query,
            evidence,
            relationships,
            search_type: recall_search_type(arguments),
            synced_dataset_count: datasets.len(),
            datasets,
        })
    }

    pub(crate) fn sync(
        &self,
        client: &CogneeClient,
        arguments: &Value,
    ) -> Result<Vec<ReadDataset>> {
        let datasets = target_datasets(client, arguments)?;
        let mut connection = self.ready_connection()?;
        for dataset in &datasets {
            sync_dataset(client, &mut connection, dataset)?;
        }
        Ok(datasets)
    }

    fn local_datasets(&self, arguments: &Value) -> Result<Vec<ReadDataset>> {
        let connection = self.ready_connection()?;
        local_target_datasets(&connection, arguments)
    }

    fn ranked_evidence(
        &self,
        arguments: &Value,
        query: &str,
        datasets: &[ReadDataset],
    ) -> Result<Vec<RecallEvidence>> {
        let candidates = self.evidence_candidates(datasets)?;
        let query_words = query_words(query);
        let top_k = optional_u64(arguments, "top_k", 10) as usize;
        Ok(ranked_candidates(&candidates, query, &query_words, top_k))
    }

    fn ranked_relationships(
        &self,
        arguments: &Value,
        query: &str,
        datasets: &[ReadDataset],
    ) -> Result<Vec<RecallRelationship>> {
        let candidates = self.relationship_candidates(datasets)?;
        let query_words = query_words(query);
        let limit = optional_u64(arguments, "top_k", 10).clamp(1, 10) as usize;
        Ok(ranked_relationships(
            &candidates,
            query,
            &query_words,
            limit,
        ))
    }

    fn evidence_candidates(&self, datasets: &[ReadDataset]) -> Result<Vec<EvidenceCandidate>> {
        let connection = self.connection()?;
        let mut candidates = data_candidates(&connection, datasets)?;
        candidates.extend(node_candidates(&connection, datasets)?);
        candidates.extend(edge_candidates(&connection, datasets)?);
        Ok(candidates)
    }

    fn relationship_candidates(
        &self,
        datasets: &[ReadDataset],
    ) -> Result<Vec<RelationshipCandidate>> {
        let connection = self.connection()?;
        relationships_for_datasets(&connection, datasets)
    }

    fn ensure_schema(&self, connection: &Connection) -> Result<()> {
        create_dataset_table(connection)?;
        create_data_table(connection)?;
        create_node_table(connection)?;
        create_edge_table(connection)?;
        Ok(())
    }

    fn ready_connection(&self) -> Result<Connection> {
        let connection = self.connection()?;
        self.ensure_schema(&connection)?;
        Ok(connection)
    }

    fn connection(&self) -> Result<Connection> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        Connection::open(&self.path).with_context(|| format!("open {:?}", self.path))
    }
}

fn create_dataset_table(connection: &Connection) -> Result<()> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS datasets(id TEXT PRIMARY KEY, name TEXT, synced_at INTEGER)",
        [],
    )?;
    Ok(())
}

fn create_data_table(connection: &Connection) -> Result<()> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS data_items(dataset_id TEXT, data_id TEXT, name TEXT, mime TEXT, raw_text TEXT, PRIMARY KEY(dataset_id, data_id))",
        [],
    )?;
    Ok(())
}

fn create_node_table(connection: &Connection) -> Result<()> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS nodes(dataset_id TEXT, node_id TEXT, label TEXT, node_type TEXT, body TEXT, PRIMARY KEY(dataset_id, node_id))",
        [],
    )?;
    Ok(())
}

fn create_edge_table(connection: &Connection) -> Result<()> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS edges(dataset_id TEXT, source_id TEXT, target_id TEXT, source_label TEXT, target_label TEXT, label TEXT)",
        [],
    )?;
    Ok(())
}

fn count_rows(connection: &Connection, table: &str) -> Result<u64> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = connection.query_row(&sql, [], |row| row.get(0))?;
    Ok(count.max(0) as u64)
}

fn target_datasets(client: &CogneeClient, arguments: &Value) -> Result<Vec<ReadDataset>> {
    let known = known_datasets(client)?;
    if let Some(id) = crate::arguments::optional_string(arguments, "dataset_id") {
        return Ok(datasets_by_id(&known, &[id]));
    }
    if let Some(name) = crate::arguments::optional_string(arguments, "dataset_name") {
        return datasets_by_name(&known, &[name]);
    }
    if let Some(ids) = optional_strings(arguments, "dataset_ids") {
        return Ok(datasets_by_id(&known, &ids));
    }
    if let Some(names) = optional_strings(arguments, "datasets") {
        return datasets_by_name(&known, &names);
    }
    Ok(known)
}

fn known_datasets(client: &CogneeClient) -> Result<Vec<ReadDataset>> {
    let datasets = client.get_json("/api/v1/datasets")?;
    Ok(datasets
        .as_array()
        .map(|values| known_dataset_rows(values))
        .unwrap_or_default())
}

fn known_dataset_rows(values: &[Value]) -> Vec<ReadDataset> {
    values.iter().map(known_dataset).collect()
}

fn known_dataset(value: &Value) -> ReadDataset {
    ReadDataset {
        id: value_text(value, "id"),
        name: value_text(value, "name"),
    }
}

fn local_target_datasets(connection: &Connection, arguments: &Value) -> Result<Vec<ReadDataset>> {
    let known = local_datasets(connection)?;
    if known.is_empty() {
        return Ok(Vec::new());
    }
    if let Some(id) = crate::arguments::optional_string(arguments, "dataset_id") {
        return Ok(local_datasets_by_id(&known, &[id]));
    }
    if let Some(name) = crate::arguments::optional_string(arguments, "dataset_name") {
        return Ok(local_datasets_by_name(&known, &[name]));
    }
    if let Some(ids) = optional_strings(arguments, "dataset_ids") {
        return Ok(local_datasets_by_id(&known, &ids));
    }
    if let Some(names) = optional_strings(arguments, "datasets") {
        return Ok(local_datasets_by_name(&known, &names));
    }
    Ok(known)
}

fn local_datasets(connection: &Connection) -> Result<Vec<ReadDataset>> {
    let mut statement = connection.prepare("SELECT id, name FROM datasets ORDER BY name")?;
    let rows = statement.query_map([], |row| {
        Ok(ReadDataset {
            id: row.get(0)?,
            name: row.get(1)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn local_datasets_by_id(known: &[ReadDataset], ids: &[String]) -> Vec<ReadDataset> {
    ids.iter()
        .filter_map(|id| known.iter().find(|dataset| dataset.id == *id).cloned())
        .collect()
}

fn local_datasets_by_name(known: &[ReadDataset], names: &[String]) -> Vec<ReadDataset> {
    names
        .iter()
        .filter_map(|name| known.iter().find(|dataset| dataset.name == *name).cloned())
        .collect()
}

fn data_items_for_datasets(
    connection: &Connection,
    datasets: &[ReadDataset],
) -> Result<Vec<ReadDataItem>> {
    let mut items = Vec::new();
    for dataset in datasets {
        items.extend(data_items_for_dataset(connection, dataset)?);
    }
    Ok(items)
}

fn data_items_for_dataset(
    connection: &Connection,
    dataset: &ReadDataset,
) -> Result<Vec<ReadDataItem>> {
    let mut statement = connection.prepare(
        "SELECT dataset_id, data_id, name, mime, raw_text FROM data_items WHERE dataset_id = ?1 ORDER BY name",
    )?;
    let rows = statement.query_map(params![dataset.id], read_data_item)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn data_item_by_id(connection: &Connection, data_id: &str) -> Result<Option<ReadDataItem>> {
    connection
        .query_row(
            "SELECT dataset_id, data_id, name, mime, raw_text FROM data_items WHERE data_id = ?1",
            params![data_id],
            read_data_item,
        )
        .optional()
        .map_err(Into::into)
}

fn read_data_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReadDataItem> {
    Ok(ReadDataItem {
        dataset_id: row.get(0)?,
        data_id: row.get(1)?,
        name: row.get(2)?,
        mime: row.get(3)?,
        raw_text: row.get(4)?,
    })
}

fn nodes_for_datasets(connection: &Connection, datasets: &[ReadDataset]) -> Result<Vec<ReadNode>> {
    let mut nodes = Vec::new();
    for dataset in datasets {
        nodes.extend(nodes_for_dataset(connection, dataset)?);
    }
    Ok(nodes)
}

fn nodes_for_dataset(connection: &Connection, dataset: &ReadDataset) -> Result<Vec<ReadNode>> {
    let mut statement = connection.prepare(
        "SELECT node_id, label, node_type, body FROM nodes WHERE dataset_id = ?1 ORDER BY label",
    )?;
    let rows = statement.query_map(params![dataset.id], read_node)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn read_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReadNode> {
    Ok(ReadNode {
        id: row.get(0)?,
        label: row.get(1)?,
        node_type: row.get(2)?,
        body: row.get(3)?,
    })
}

fn edges_for_datasets(connection: &Connection, datasets: &[ReadDataset]) -> Result<Vec<ReadEdge>> {
    let mut edges = Vec::new();
    for dataset in datasets {
        edges.extend(edges_for_dataset(connection, dataset)?);
    }
    Ok(edges)
}

fn edges_for_dataset(connection: &Connection, dataset: &ReadDataset) -> Result<Vec<ReadEdge>> {
    let mut statement = connection.prepare(
        "SELECT source_id, target_id, source_label, target_label, label FROM edges WHERE dataset_id = ?1 ORDER BY source_label, target_label",
    )?;
    let rows = statement.query_map(params![dataset.id], read_edge)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn read_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReadEdge> {
    Ok(ReadEdge {
        source_id: row.get(0)?,
        target_id: row.get(1)?,
        source_label: row.get(2)?,
        target_label: row.get(3)?,
        label: row.get(4)?,
    })
}

fn forget_data_item(connection: &Connection, data_id: &str) -> Result<LocalForget> {
    let deleted_data_items = connection.execute(
        "DELETE FROM data_items WHERE data_id = ?1",
        params![data_id],
    )? as u64;
    Ok(LocalForget {
        deleted_datasets: 0,
        deleted_data_items,
        deleted_nodes: 0,
        deleted_edges: 0,
    })
}

fn forget_datasets(connection: &Connection, datasets: &[ReadDataset]) -> Result<LocalForget> {
    let mut result = LocalForget {
        deleted_datasets: 0,
        deleted_data_items: 0,
        deleted_nodes: 0,
        deleted_edges: 0,
    };

    for dataset in datasets {
        result.deleted_data_items += connection.execute(
            "DELETE FROM data_items WHERE dataset_id = ?1",
            params![dataset.id],
        )? as u64;
        result.deleted_nodes += connection.execute(
            "DELETE FROM nodes WHERE dataset_id = ?1",
            params![dataset.id],
        )? as u64;
        result.deleted_edges += connection.execute(
            "DELETE FROM edges WHERE dataset_id = ?1",
            params![dataset.id],
        )? as u64;
        result.deleted_datasets +=
            connection.execute("DELETE FROM datasets WHERE id = ?1", params![dataset.id])? as u64;
    }

    Ok(result)
}

fn datasets_by_id(known: &[ReadDataset], ids: &[String]) -> Vec<ReadDataset> {
    ids.iter()
        .map(|id| dataset_by_id(known, id))
        .collect::<Vec<_>>()
}

fn dataset_by_id(known: &[ReadDataset], id: &str) -> ReadDataset {
    known
        .iter()
        .find(|dataset| dataset.id == id)
        .cloned()
        .unwrap_or_else(|| ReadDataset {
            id: id.to_string(),
            name: String::new(),
        })
}

fn datasets_by_name(known: &[ReadDataset], names: &[String]) -> Result<Vec<ReadDataset>> {
    names
        .iter()
        .map(|name| dataset_by_name(known, name))
        .collect::<Result<Vec<_>>>()
}

fn dataset_by_name(known: &[ReadDataset], name: &str) -> Result<ReadDataset> {
    known
        .iter()
        .find(|dataset| dataset.name == name)
        .cloned()
        .ok_or_else(|| anyhow!("dataset not found: {name}"))
}

fn sync_dataset(
    client: &CogneeClient,
    connection: &mut Connection,
    dataset: &ReadDataset,
) -> Result<()> {
    let graph = client.get_json(&format!("/api/v1/datasets/{}/graph", dataset.id))?;
    let data_items = client.get_json(&format!("/api/v1/datasets/{}/data", dataset.id))?;
    let transaction = connection.transaction()?;
    replace_dataset_rows(&transaction, dataset)?;
    insert_graph_rows(&transaction, dataset, &graph)?;
    insert_data_rows(client, &transaction, dataset, &data_items)?;
    transaction.commit()?;
    Ok(())
}

fn replace_dataset_rows(connection: &Connection, dataset: &ReadDataset) -> Result<()> {
    delete_dataset_rows(connection, dataset)?;
    connection.execute(
        "INSERT OR REPLACE INTO datasets(id, name, synced_at) VALUES (?1, ?2, ?3)",
        params![dataset.id, dataset.name, unix_timestamp()],
    )?;
    Ok(())
}

fn delete_dataset_rows(connection: &Connection, dataset: &ReadDataset) -> Result<()> {
    connection.execute(
        "DELETE FROM data_items WHERE dataset_id = ?1",
        params![dataset.id],
    )?;
    connection.execute(
        "DELETE FROM nodes WHERE dataset_id = ?1",
        params![dataset.id],
    )?;
    connection.execute(
        "DELETE FROM edges WHERE dataset_id = ?1",
        params![dataset.id],
    )?;
    Ok(())
}

fn insert_graph_rows(connection: &Connection, dataset: &ReadDataset, graph: &Value) -> Result<()> {
    let node_labels = graph_node_labels(graph);
    insert_node_rows(connection, dataset, graph)?;
    insert_edge_rows(connection, dataset, graph, &node_labels)?;
    Ok(())
}

fn graph_node_labels(graph: &Value) -> HashMap<String, String> {
    graph
        .get("nodes")
        .and_then(Value::as_array)
        .map(|nodes| node_label_map(nodes))
        .unwrap_or_default()
}

fn node_label_map(nodes: &[Value]) -> HashMap<String, String> {
    nodes
        .iter()
        .map(|node| (value_text(node, "id"), value_text(node, "label")))
        .collect()
}

fn insert_node_rows(connection: &Connection, dataset: &ReadDataset, graph: &Value) -> Result<()> {
    for node in graph_array(graph, "nodes") {
        insert_node_row(connection, dataset, node)?;
    }
    Ok(())
}

fn insert_node_row(connection: &Connection, dataset: &ReadDataset, node: &Value) -> Result<()> {
    connection.execute(
        "INSERT OR REPLACE INTO nodes(dataset_id, node_id, label, node_type, body) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![dataset.id, value_text(node, "id"), value_text(node, "label"), value_text(node, "type"), node_body(node)],
    )?;
    Ok(())
}

fn node_body(node: &Value) -> String {
    let properties = node.get("properties").unwrap_or(&Value::Null);
    [
        value_text(properties, "text"),
        value_text(properties, "description"),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

fn insert_edge_rows(
    connection: &Connection,
    dataset: &ReadDataset,
    graph: &Value,
    labels: &HashMap<String, String>,
) -> Result<()> {
    for edge in graph_array(graph, "edges") {
        insert_edge_row(connection, dataset, edge, labels)?;
    }
    Ok(())
}

fn insert_edge_row(
    connection: &Connection,
    dataset: &ReadDataset,
    edge: &Value,
    labels: &HashMap<String, String>,
) -> Result<()> {
    connection.execute(
        "INSERT INTO edges(dataset_id, source_id, target_id, source_label, target_label, label) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![dataset.id, edge_id(edge, "source"), edge_id(edge, "target"), edge_label(edge, labels, "source"), edge_label(edge, labels, "target"), value_text(edge, "label")],
    )?;
    Ok(())
}

fn insert_data_rows(
    client: &CogneeClient,
    connection: &Connection,
    dataset: &ReadDataset,
    data_items: &Value,
) -> Result<()> {
    for item in data_items.as_array().into_iter().flatten() {
        insert_data_row(client, connection, dataset, item)?;
    }
    Ok(())
}

fn insert_data_row(
    client: &CogneeClient,
    connection: &Connection,
    dataset: &ReadDataset,
    item: &Value,
) -> Result<()> {
    connection.execute(
        "INSERT OR REPLACE INTO data_items(dataset_id, data_id, name, mime, raw_text) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![dataset.id, value_text(item, "id"), value_text(item, "name"), value_text(item, "mimeType"), raw_text(client, dataset, item)],
    )?;
    Ok(())
}

fn raw_text(client: &CogneeClient, dataset: &ReadDataset, item: &Value) -> String {
    client
        .get_text(&format!(
            "/api/v1/datasets/{}/data/{}/raw",
            dataset.id,
            value_text(item, "id")
        ))
        .unwrap_or_default()
}

fn graph_array<'a>(graph: &'a Value, key: &str) -> &'a [Value] {
    graph
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn edge_id(edge: &Value, key: &str) -> String {
    value_text(edge, key)
}

fn edge_label(edge: &Value, labels: &HashMap<String, String>, key: &str) -> String {
    labels.get(&edge_id(edge, key)).cloned().unwrap_or_default()
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn data_candidates(
    connection: &Connection,
    datasets: &[ReadDataset],
) -> Result<Vec<EvidenceCandidate>> {
    let mut candidates = Vec::new();
    for dataset in datasets {
        candidates.extend(data_candidates_for_dataset(connection, dataset)?);
    }
    Ok(candidates)
}

fn data_candidates_for_dataset(
    connection: &Connection,
    dataset: &ReadDataset,
) -> Result<Vec<EvidenceCandidate>> {
    let mut statement = connection
        .prepare("SELECT data_id, name, raw_text FROM data_items WHERE dataset_id = ?1")?;
    let rows = statement.query_map(params![dataset.id], data_candidate)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn data_candidate(row: &rusqlite::Row<'_>) -> rusqlite::Result<EvidenceCandidate> {
    Ok(EvidenceCandidate {
        source_kind: "raw_data".to_string(),
        handle: format!("data_id: {}", row.get::<_, String>(0)?),
        label: row.get(1)?,
        text: row.get(2)?,
    })
}

fn node_candidates(
    connection: &Connection,
    datasets: &[ReadDataset],
) -> Result<Vec<EvidenceCandidate>> {
    let mut candidates = Vec::new();
    for dataset in datasets {
        candidates.extend(node_candidates_for_dataset(connection, dataset)?);
    }
    Ok(candidates)
}

fn node_candidates_for_dataset(
    connection: &Connection,
    dataset: &ReadDataset,
) -> Result<Vec<EvidenceCandidate>> {
    let mut statement = connection
        .prepare("SELECT node_id, label, node_type, body FROM nodes WHERE dataset_id = ?1")?;
    let rows = statement.query_map(params![dataset.id], node_candidate)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn node_candidate(row: &rusqlite::Row<'_>) -> rusqlite::Result<EvidenceCandidate> {
    let label: String = row.get(1)?;
    Ok(EvidenceCandidate {
        source_kind: row.get(2)?,
        handle: format!("node_id: {}", row.get::<_, String>(0)?),
        text: candidate_text(&label, &row.get::<_, String>(3)?),
        label,
    })
}

fn edge_candidates(
    connection: &Connection,
    datasets: &[ReadDataset],
) -> Result<Vec<EvidenceCandidate>> {
    let mut candidates = Vec::new();
    for dataset in datasets {
        candidates.extend(edge_candidates_for_dataset(connection, dataset)?);
    }
    Ok(candidates)
}

fn edge_candidates_for_dataset(
    connection: &Connection,
    dataset: &ReadDataset,
) -> Result<Vec<EvidenceCandidate>> {
    let mut statement = connection.prepare(
        "SELECT source_id, target_id, source_label, target_label, label FROM edges WHERE dataset_id = ?1",
    )?;
    let rows = statement.query_map(params![dataset.id], edge_candidate)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn edge_candidate(row: &rusqlite::Row<'_>) -> rusqlite::Result<EvidenceCandidate> {
    let source: String = row.get(2)?;
    let target: String = row.get(3)?;
    let label: String = row.get(4)?;
    Ok(EvidenceCandidate {
        source_kind: "graph_edge".to_string(),
        handle: format!(
            "edge: {} -> {}",
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?
        ),
        text: format!("{source} --[{label}]--> {target}"),
        label,
    })
}

fn relationships_for_datasets(
    connection: &Connection,
    datasets: &[ReadDataset],
) -> Result<Vec<RelationshipCandidate>> {
    let mut candidates = Vec::new();
    for dataset in datasets {
        candidates.extend(relationships_for_dataset(connection, dataset)?);
    }
    Ok(candidates)
}

fn relationships_for_dataset(
    connection: &Connection,
    dataset: &ReadDataset,
) -> Result<Vec<RelationshipCandidate>> {
    let mut statement = connection.prepare(
        "SELECT source_id, target_id, source_label, target_label, label FROM edges WHERE dataset_id = ?1",
    )?;
    let rows = statement.query_map(params![dataset.id], relationship_candidate)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn relationship_candidate(row: &rusqlite::Row<'_>) -> rusqlite::Result<RelationshipCandidate> {
    Ok(RelationshipCandidate {
        source_id: row.get(0)?,
        target_id: row.get(1)?,
        source: row.get(2)?,
        target: row.get(3)?,
        relationship: row.get(4)?,
    })
}

fn candidate_text(label: &str, body: &str) -> String {
    [label, body]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn ranked_candidates(
    candidates: &[EvidenceCandidate],
    query: &str,
    words: &[String],
    top_k: usize,
) -> Vec<RecallEvidence> {
    let mut scored = scored_candidates(candidates, query, words);
    scored.sort_by(|left, right| right.0.cmp(&left.0));
    scored
        .into_iter()
        .take(top_k)
        .enumerate()
        .map(ranked_evidence)
        .collect()
}

fn ranked_relationships(
    candidates: &[RelationshipCandidate],
    query: &str,
    words: &[String],
    limit: usize,
) -> Vec<RecallRelationship> {
    let mut scored = scored_relationships(candidates, query, words);
    if scored.iter().all(|(score, _)| *score == 0) {
        scored.truncate(limit);
    } else {
        scored.retain(|(score, _)| *score > 0);
        scored.sort_by(|left, right| right.0.cmp(&left.0));
    }

    scored
        .into_iter()
        .take(limit)
        .enumerate()
        .map(ranked_relationship)
        .collect()
}

fn scored_relationships<'a>(
    candidates: &'a [RelationshipCandidate],
    query: &str,
    words: &[String],
) -> Vec<(u64, &'a RelationshipCandidate)> {
    candidates
        .iter()
        .map(|candidate| (relationship_score(candidate, query, words), candidate))
        .collect()
}

fn ranked_relationship(
    (index, (score, candidate)): (usize, (u64, &RelationshipCandidate)),
) -> RecallRelationship {
    RecallRelationship {
        rank: index + 1,
        score,
        source: candidate.source.clone(),
        relationship: candidate.relationship.clone(),
        target: candidate.target.clone(),
        handle: format!("edge: {} -> {}", candidate.source_id, candidate.target_id),
    }
}

fn scored_candidates<'a>(
    candidates: &'a [EvidenceCandidate],
    query: &str,
    words: &[String],
) -> Vec<(u64, &'a EvidenceCandidate)> {
    candidates
        .iter()
        .map(|candidate| (candidate_score(candidate, query, words), candidate))
        .filter(|(score, _)| *score > 0)
        .collect()
}

fn ranked_evidence(
    (index, (score, candidate)): (usize, (u64, &EvidenceCandidate)),
) -> RecallEvidence {
    RecallEvidence {
        rank: index + 1,
        score,
        source_kind: candidate.source_kind.clone(),
        label: candidate.label.clone(),
        text: truncate(&candidate.text, 700),
        handle: candidate.handle.clone(),
    }
}

fn candidate_score(candidate: &EvidenceCandidate, query: &str, words: &[String]) -> u64 {
    let haystack = candidate_haystack(candidate);
    let word_score = words.iter().filter(|word| haystack.contains(*word)).count() as u64;
    word_score * 10 + exact_phrase_score(&haystack, query)
}

fn exact_phrase_score(haystack: &str, query: &str) -> u64 {
    if haystack.contains(&query.to_lowercase()) {
        25
    } else {
        0
    }
}

fn candidate_haystack(candidate: &EvidenceCandidate) -> String {
    format!(
        "{} {} {}",
        candidate.label, candidate.source_kind, candidate.text
    )
    .to_lowercase()
}

fn relationship_score(candidate: &RelationshipCandidate, query: &str, words: &[String]) -> u64 {
    let haystack = relationship_haystack(candidate);
    let word_score = words.iter().filter(|word| haystack.contains(*word)).count() as u64;
    word_score * 10 + exact_phrase_score(&haystack, query)
}

fn relationship_haystack(candidate: &RelationshipCandidate) -> String {
    format!(
        "{} {} {}",
        candidate.source, candidate.relationship, candidate.target
    )
    .to_lowercase()
}

fn query_words(query: &str) -> Vec<String> {
    query
        .split(|character: char| !character.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|word| word.len() > 1)
        .collect()
}

fn recall_search_type(arguments: &Value) -> String {
    crate::arguments::optional_string(arguments, "search_type")
        .unwrap_or_else(|| "GRAPH_COMPLETION".to_string())
}
