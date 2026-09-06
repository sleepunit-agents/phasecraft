//! Phrase inheritance and section expansion happen before playback, using normal TOML tables.
use serde::Deserialize;
use toml::{Table, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sequence {
    #[serde(default)]
    repeat: bool,
    sections: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    phrase: String,
    bars: Option<u32>,
    #[serde(default)]
    phase: crate::music::arrangement::PhasePolicy,
    #[serde(default = "once")]
    repeat: u32,
    composition: Option<Value>,
}
fn once() -> u32 {
    1
}
fn derive(
    name: &str,
    definitions: &Table,
    base: &Value,
    stack: &mut Vec<String>,
) -> Result<Value, String> {
    if stack.len() >= 16 || stack.iter().any(|s| s == name) {
        return Err(format!(
            "phrase dependency cycle or excessive depth: {} -> {name}",
            stack.join(" -> ")
        ));
    }
    let mut local = definitions
        .get(name)
        .and_then(Value::as_table)
        .ok_or_else(|| format!("unknown phrase {name:?}"))?
        .clone();
    if local
        .keys()
        .any(|k| !["use", "seed", "phrase_bars", "parts", "part", "accents"].contains(&k.as_str()))
    {
        return Err(format!(
            "phrase {name:?} only accepts use, seed, phrase_bars, parts and accents"
        ));
    }
    stack.push(name.into());
    let mut result = if let Some(parent) = local.remove("use") {
        derive(
            parent.as_str().ok_or("phrase.use must be a phrase name")?,
            definitions,
            base,
            stack,
        )?
    } else {
        base.clone()
    };
    stack.pop();
    super::library::merge(&mut result, Value::Table(local));
    Ok(result)
}
pub(super) fn expand(
    mut root: Table,
    flat: &dyn Fn(Table) -> Result<Value, String>,
) -> Result<Value, String> {
    let definitions = root.remove("phrases").unwrap_or(Value::Table(Table::new()));
    let definitions = definitions
        .as_table()
        .ok_or("phrases must be named tables")?;
    if definitions.len() > 64 || definitions.keys().any(|k| k.trim().is_empty()) {
        return Err("phrases requires at most 64 nonempty names".into());
    }
    let sequence = root.remove("arrangement");
    let base = Value::Table(root.clone());
    // Validate every definition, even if a sequence does not use it yet.
    let mut phrases = std::collections::BTreeMap::new();
    for name in definitions.keys() {
        let raw = derive(name, definitions, &base, &mut vec![])?;
        let value =
            flat(raw.as_table().unwrap().clone()).map_err(|e| format!("phrase {name:?}: {e}"))?;
        let _: crate::music::Composition = value
            .clone()
            .try_into()
            .map_err(|e: toml::de::Error| e.to_string())?;
        phrases.insert(name.clone(), value);
    }
    let mut result = flat(root)?;
    if let Some(sequence) = sequence {
        let sequence: Sequence = sequence
            .try_into()
            .map_err(|e: toml::de::Error| e.to_string())?;
        if sequence.sections.is_empty() || sequence.sections.len() > 64 {
            return Err("arrangement requires 1..64 sections".into());
        }
        let mut sections = vec![];
        for entry in sequence.sections {
            if entry.repeat == 0 || entry.repeat > 64 || sections.len() + entry.repeat as usize > 64
            {
                return Err(
                    "expanded arrangement requires 1..64 sections; entry repeat must be 1..64"
                        .into(),
                );
            }
            let value = if let Some(value) = entry.composition {
                if definitions.contains_key(&entry.phrase) {
                    return Err(
                        "choose a phrase reference or expanded composition, not both".into(),
                    );
                }
                flat(
                    value
                        .as_table()
                        .ok_or("section.composition must be a table")?
                        .clone(),
                )?
            } else {
                phrases
                    .get(&entry.phrase)
                    .ok_or_else(|| format!("unknown arrangement phrase {:?}", entry.phrase))?
                    .clone()
            };
            let bars = entry.bars.unwrap_or(
                value
                    .get("phrase_bars")
                    .and_then(Value::as_integer)
                    .unwrap_or(4) as u32,
            );
            for _ in 0..entry.repeat {
                let mut section = Table::new();
                section.insert("phrase".into(), Value::String(entry.phrase.clone()));
                section.insert("bars".into(), Value::Integer(i64::from(bars)));
                section.insert(
                    "phase".into(),
                    Value::try_from(entry.phase).map_err(|e| e.to_string())?,
                );
                section.insert("composition".into(), value.clone());
                sections.push(Value::Table(section));
            }
        }
        result.as_table_mut().unwrap().insert(
            "arrangement".into(),
            Value::Table(Table::from_iter([
                ("repeat".into(), Value::Boolean(sequence.repeat)),
                ("sections".into(), Value::Array(sections)),
            ])),
        );
    }
    Ok(result)
}
