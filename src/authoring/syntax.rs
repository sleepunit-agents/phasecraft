//! Small TOML conveniences, normalized before inheritance so kind changes are explicit.
use toml::{Table, Value};

pub(super) fn rhythm(value: &mut Value) -> Result<(), String> {
    let fields = value.as_table_mut().ok_or("rhythm must be a table")?;
    if let Some(part) = fields.remove("part") {
        if fields.contains_key("id")
            || fields
                .get("type")
                .is_some_and(|v| v.as_str() != Some("part"))
        {
            return Err("rhythm.part conflicts with id or type".into());
        }
        fields.insert("id".into(), part);
        fields.insert("type".into(), Value::String("part".into()));
    }
    if !fields.contains_key("type") {
        let kind = if fields.contains_key("op") {
            Some("binary")
        } else if fields.contains_key("steps") {
            Some("euclidean")
        } else {
            None
        }; // A partial override inherits the existing kind.
        if let Some(kind) = kind {
            fields.insert("type".into(), Value::String(kind.into()));
        }
    }
    for child in ["a", "b"] {
        if let Some(value) = fields.get_mut(child) {
            rhythm(value)?;
        }
    }
    Ok(())
}

pub(super) fn behavior(fields: &mut Table) -> Result<(), String> {
    for lane in ["trigger", "accent"] {
        if let Some(value) = fields
            .get_mut(lane)
            .and_then(Value::as_table_mut)
            .and_then(|t| t.get_mut("rhythm"))
        {
            rhythm(value).map_err(|e| format!("{lane}.rhythm: {e}"))?;
        }
    }
    Ok(())
}

pub(super) fn keyed_parts(root: &mut Table) -> Result<(), String> {
    if let Some(Value::Table(parts)) = root.get_mut("parts") {
        let mut result = Vec::new();
        for (id, value) in parts.iter() {
            let mut fields = value
                .as_table()
                .ok_or_else(|| format!("parts.{id} must be a table"))?
                .clone();
            if fields.contains_key("id") {
                return Err(format!(
                    "parts.{id}: the table name supplies id; remove the id field"
                ));
            }
            fields.insert("id".into(), Value::String(id.clone()));
            result.push(Value::Table(fields));
        }
        root.insert("parts".into(), Value::Array(result));
    }
    Ok(())
}
