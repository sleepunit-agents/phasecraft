//! Authoring-only expansion. The scheduler receives a validated concrete model.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use toml::{Table, Value};

#[derive(Default)]
struct Registry {
    behaviors: BTreeMap<String, Value>,
    profiles: BTreeMap<String, Value>,
}

fn table(value: &Value) -> Result<&Table, String> {
    value
        .as_table()
        .ok_or_else(|| "expected a TOML table".into())
}
fn names(value: Value, field: &str) -> Result<Vec<String>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("{field} must be an array of names"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{field} entries must be strings"))
        })
        .collect()
}
fn merge(base: &mut Value, overlay: Value) {
    if let (Value::Table(a), Value::Table(b)) = (&mut *base, &overlay) {
        // Switching node/profile kinds discards fields belonging to the old kind.
        let new_kind = b
            .get("type")
            .is_some_and(|kind| a.get("type") != Some(kind));
        if !new_kind && !b.contains_key("use") {
            for (key, value) in b {
                if let Some(existing) = a.get_mut(key) {
                    merge(existing, value.clone());
                } else {
                    a.insert(key.clone(), value.clone());
                }
            }
            return;
        }
    }
    *base = overlay;
}
impl Registry {
    fn add(&mut self, library: Value) -> Result<(), String> {
        for (kind, entries) in table(&library)? {
            let target = match kind.as_str() {
                "behaviors" => &mut self.behaviors,
                "profiles" => &mut self.profiles,
                _ => return Err(format!("unknown library section {kind:?}")),
            };
            for (name, body) in table(entries)? {
                if name.trim().is_empty() {
                    return Err("library names cannot be empty".into());
                }
                table(body)?;
                if target.insert(name.clone(), body.clone()).is_some() {
                    return Err(format!("duplicate library definition {kind}.{name}"));
                }
            }
        }
        Ok(())
    }
    fn expand(
        &self,
        value: &Value,
        profile: bool,
        stack: &mut Vec<String>,
    ) -> Result<Value, String> {
        let mut local = table(value)?.clone();
        if !profile {
            super::syntax::behavior(&mut local)?;
        }
        let used = local.remove("use");
        let composed = local.remove("compose");
        let refs = match (used, composed) {
            (Some(_), Some(_)) => return Err("choose use or compose, not both".into()),
            (Some(value), None) => vec![value.as_str().ok_or("use must be a name")?.to_owned()],
            (None, Some(value)) => names(value, "compose")?,
            (None, None) => vec![],
        };
        let mut result = Value::Table(Table::new());
        for name in refs {
            if stack.len() >= 32 || stack.contains(&name) {
                return Err(format!(
                    "library dependency cycle or excessive depth: {} -> {name}",
                    stack.join(" -> ")
                ));
            }
            let registry = if profile {
                &self.profiles
            } else {
                &self.behaviors
            };
            let definition = registry.get(&name).ok_or_else(|| {
                format!(
                    "unknown {} {name:?}",
                    if profile { "profile" } else { "behavior" }
                )
            })?;
            stack.push(name);
            let expanded = self.expand(definition, profile, stack)?;
            stack.pop();
            merge(&mut result, expanded);
        }
        merge(&mut result, Value::Table(local));
        Ok(result)
    }
}
fn load_library(
    path: &Path,
    registry: &mut Registry,
    active: &mut Vec<PathBuf>,
    loaded: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if active.contains(&path) || active.len() >= 16 {
        return Err(format!(
            "library import cycle or excessive depth at {}",
            path.display()
        ));
    }
    if loaded.contains(&path) {
        return Ok(());
    }
    active.push(path.clone());
    let source = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut file: Table =
        toml::from_str(&source).map_err(|e| format!("{}: {e}", path.display()))?;
    imports(
        &mut file,
        Some(path.parent().unwrap()),
        registry,
        active,
        loaded,
    )?;
    if let Some(library) = file.remove("library") {
        registry.add(library)?;
    }
    if !file.is_empty() {
        return Err(format!(
            "library file {} only accepts imports and library",
            path.display()
        ));
    }
    active.pop();
    loaded.push(path);
    Ok(())
}
fn imports(
    file: &mut Table,
    base: Option<&Path>,
    registry: &mut Registry,
    active: &mut Vec<PathBuf>,
    loaded: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if let Some(value) = file.remove("imports") {
        for name in names(value, "imports")? {
            let base = base.ok_or("file imports require loading a composition from disk")?;
            load_library(&base.join(name), registry, active, loaded)?;
        }
    }
    Ok(())
}

pub fn expand(source: &str, base: Option<&Path>) -> Result<Value, String> {
    expand_with_libraries(source, base, &[])
}

pub(super) fn expand_with_libraries(
    source: &str,
    base: Option<&Path>,
    libraries: &[PathBuf],
) -> Result<Value, String> {
    let mut registry = Registry::default();
    for source in [
        include_str!("../../library/drums/common.toml"),
        include_str!("../../library/drums/techno.toml"),
        include_str!("../../library/drums/dnb.toml"),
        include_str!("../../library/accents/velocity.toml"),
        include_str!("../../library/kits/909.toml"),
    ] {
        registry.add(toml::from_str(source).map_err(|e| format!("built-in library: {e}"))?)?;
    }
    let mut root: Table = toml::from_str(source).map_err(|e| e.to_string())?;
    let mut loaded = vec![];
    for path in libraries {
        load_library(path, &mut registry, &mut vec![], &mut loaded)?;
    }
    imports(&mut root, base, &mut registry, &mut vec![], &mut loaded)?;
    super::syntax::keyed_parts(&mut root)?;
    if let Some(library) = root.remove("library") {
        registry.add(library)?;
    }
    fn part(registry: &Registry, value: &Value) -> Result<Value, String> {
        let mut expanded = registry.expand(value, false, &mut vec![])?;
        // Identity belongs to the composition, never a reusable definition.
        let id = table(value)?
            .get("id")
            .ok_or("every Part instance needs its own id")?
            .clone();
        let fields = expanded.as_table_mut().unwrap();
        fields.insert("id".into(), id);
        if let Some(profile) = fields.get_mut("profile") {
            *profile = registry.expand(profile, true, &mut vec![])?;
        }
        let _: crate::music::Part = expanded
            .clone()
            .try_into()
            .map_err(|e: toml::de::Error| e.to_string())?;
        Ok(expanded)
    }
    fn fields_id(value: &Value) -> &str {
        value.get("id").and_then(Value::as_str).unwrap_or("?")
    }
    if let Some(value) = root.get_mut("part") {
        *value = part(&registry, value).map_err(|e| format!("parts.{}: {e}", fields_id(value)))?;
    }
    if let Some(value) = root.get_mut("parts") {
        let parts = value
            .as_array_mut()
            .ok_or("parts must be an array of tables")?;
        for value in parts {
            *value =
                part(&registry, value).map_err(|e| format!("parts.{}: {e}", fields_id(value)))?;
        }
    }
    Ok(Value::Table(root))
}
