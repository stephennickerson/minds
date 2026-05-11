use serde_json::Value;

pub(crate) fn packet(
    title: &str,
    answer: &str,
    evidence: &str,
    next: &str,
    coverage: &str,
) -> String {
    format!(
        "# {title}\n\n## Answer\n\n{answer}\n\n## Evidence\n\n{evidence}\n\n## Navigate Next\n\n{next}\n\n## Source / Coverage\n\n{coverage}\n\nContinuation handles: `dataset_name`, `dataset_id`, `data_id`, `pipeline_run_id`, `search_type`, and `node_name` when Cognee returns them.\n"
    )
}

pub(crate) fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return "No rows.".to_string();
    }
    let header = markdown_row(
        &headers
            .iter()
            .map(|item| item.to_string())
            .collect::<Vec<_>>(),
    );
    let separator = markdown_row(
        &headers
            .iter()
            .map(|_| "---".to_string())
            .collect::<Vec<_>>(),
    );
    let body = rows
        .iter()
        .map(|row| markdown_row(row))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{header}\n{separator}\n{body}")
}

pub(crate) fn json_block(value: &Value) -> String {
    let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    format!("```json\n{}\n```", truncate(&text, 4_000))
}

pub(crate) fn text_block(text: &str) -> String {
    format!("```text\n{}\n```", truncate(text, 4_000))
}

pub(crate) fn value_text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(crate) fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let head = value.chars().take(limit).collect::<String>();
    format!("{head}\n...[truncated]")
}

fn markdown_row(cells: &[String]) -> String {
    let escaped = cells
        .iter()
        .map(|cell| markdown_cell(cell))
        .collect::<Vec<_>>();
    format!("| {} |", escaped.join(" | "))
}

fn markdown_cell(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace("\r\n", "<br>")
        .replace('\n', "<br>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_has_required_sections() {
        let text = packet("Title", "A", "B", "C", "D");
        assert!(text.contains("## Answer"));
        assert!(text.contains("## Evidence"));
        assert!(text.contains("## Navigate Next"));
        assert!(text.contains("## Source / Coverage"));
    }

    #[test]
    fn table_escapes_pipes() {
        let text = table(&["A"], &[vec!["x|y".to_string()]]);
        assert!(text.contains("x\\|y"));
    }
}
