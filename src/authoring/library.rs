//! Authoring-only expansion. The scheduler receives a validated concrete model.
use serde::Serialize;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use toml::{Table, Value};

/// A voice whose instrument declared nothing, listed so the draft can still be inspected.
///
/// `[library.kit.<instrument>]` may declare an output with no `controls` table at all. That
/// instrument has declared nothing about what it can hear, so a Part bound to it through
/// `kit = "<instrument>"` that still carries parameter lanes or profile controls lands them
/// nowhere. That is a gap, not an error: `validate` and `inspect` list it and go on without
/// those controls; every playback door refuses the piece while one exists (`refuse`). A kit
/// entry that *does* declare a `controls` table, even an empty one, keeps today's refusal
/// for a name it does not list.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Gap {
    pub part: String,
    pub instrument: String,
    pub controls: Vec<String>,
}
impl std::fmt::Display for Gap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Part {:?}: instrument {:?} declares no controls, so {} lands nowhere",
            self.part,
            self.instrument,
            self.controls.join(", ")
        )
    }
}
/// A gap never authorises playback: the draft can be read, not sounded.
pub fn refuse(gaps: &[Gap]) -> Result<(), String> {
    if gaps.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} unbound control{}; inspect or validate the draft, or declare the controls in the kit before playing: {}",
        gaps.len(),
        if gaps.len() == 1 { "" } else { "s" },
        gaps.iter()
            .map(Gap::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    ))
}
/// An expanded composition together with the gaps its kit left open.
pub struct Draft {
    pub value: Value,
    pub gaps: Vec<Gap>,
}

/// Where a library section was written, so an entry that fails is reported against its own
/// file and not against the composition that happened to load it (t-505).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Origin {
    /// One of the libraries compiled into the binary, by its path in the source tree.
    BuiltIn(&'static str),
    /// A library file on disk: an import, or a project's `libraries` entry.
    File(PathBuf),
    /// The composition's own `[library]` table. Its errors carry no prefix here: the caller
    /// that loaded the composition names its path, as it does for every other error in it.
    Composition,
}
impl Origin {
    fn attach(&self, error: String) -> String {
        match self {
            Origin::BuiltIn(path) => format!("built-in library {path}: {error}"),
            Origin::File(path) => format!("{}: {error}", path.display()),
            Origin::Composition => error,
        }
    }
}

#[derive(Default)]
struct Registry {
    behaviors: BTreeMap<String, Value>,
    profiles: BTreeMap<String, Value>,
    /// The kit: instrument name -> an output table, or the name of a behavior whose output it is.
    kit: BTreeMap<String, Value>,
    /// Where each kit entry was written, for the alias check that runs once the registry is
    /// complete and so has no call site left to name the file.
    kit_origins: BTreeMap<String, Origin>,
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
/// A kit entry is a behavior name or an output table. The table is checked as an output
/// here — shape *and* range, by the same `Output::validate` a Part is held to — so a mistyped
/// kit fails where it is written, whether or not any voice ever binds to it. An alias cannot
/// be checked here: it may name a behavior a later library declares, so the registry resolves
/// every alias once it is complete (`Registry::check_aliases`).
/// The shape-and-range check for an output table, with no alias alternative. A kit entry may
/// be *declared* as a table or as a behavior name, but what an alias RESOLVES to is the output
/// itself and can only be a table — so the resolved value is checked here rather than through
/// `instrument`, whose string branch would read it as one more alias and drop it again.
fn output_table(body: &Value) -> Result<Value, String> {
    let Value::Table(output) = body else {
        return Err(format!(
            "an instrument output must be a table of output fields, not the {} {body}",
            body.type_str()
        ));
    };
    let mut holder = Table::new();
    holder.insert("output".into(), Value::Table(output.clone()));
    super::syntax::behavior(&mut holder)?;
    let output = holder.remove("output").unwrap();
    let typed: crate::music::Output = output
        .clone()
        .try_into()
        .map_err(|e: toml::de::Error| e.to_string())?;
    typed.validate()?;
    Ok(output)
}
/// One kit entry as it is *written*: an output table, or the name of a behavior to resolve
/// later (`check_aliases`). The table alternative is `output_table`; this adds the alias one.
fn instrument(body: &Value) -> Result<Value, String> {
    match body {
        Value::String(behavior) if !behavior.trim().is_empty() => Ok(body.clone()),
        Value::String(_) => Err("an instrument alias must name a behavior".into()),
        Value::Table(_) => output_table(body),
        _ => Err("an instrument is an output table or the name of a behavior".into()),
    }
}
pub(super) fn merge(base: &mut Value, overlay: Value) {
    if let (Value::Table(a), Value::Table(b)) = (&mut *base, &overlay) {
        if b.contains_key("compose") && !b.contains_key("use") {
            a.remove("use");
        }
        // Replacing a trajectory through an override must not retain its old kind.
        if b.contains_key("automation") && !b.contains_key("ramp") {
            a.remove("ramp");
        }
        if b.contains_key("ramp") && !b.contains_key("automation") {
            a.remove("automation");
        }
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
    /// Add one `[library]` table. Every error raised here names `origin`: the file the entry
    /// was written in, not the composition that loaded it.
    fn add(&mut self, library: Value, origin: &Origin) -> Result<(), String> {
        self.add_entries(library, origin)
            .map_err(|e| origin.attach(e))
    }
    fn add_entries(&mut self, library: Value, origin: &Origin) -> Result<(), String> {
        for (kind, entries) in table(&library)? {
            let target = match kind.as_str() {
                "behaviors" => &mut self.behaviors,
                "profiles" => &mut self.profiles,
                "kit" => &mut self.kit,
                _ => return Err(format!("unknown library section {kind:?}")),
            };
            for (name, body) in table(entries)? {
                if name.trim().is_empty() {
                    return Err("library names cannot be empty".into());
                }
                let body = if kind == "kit" {
                    instrument(body).map_err(|e| format!("kit.{name}: {e}"))?
                } else {
                    table(body)?;
                    body.clone()
                };
                if target.insert(name.clone(), body).is_some() {
                    return Err(format!("duplicate library definition {kind}.{name}"));
                }
                if kind == "kit" {
                    self.kit_origins.insert(name.clone(), origin.clone());
                }
            }
        }
        Ok(())
    }
    /// Resolve every alias in the kit once the registry is complete, then hold what it
    /// resolved to to the same `Output::validate` an inline table is held to. An alias is
    /// valid where it is written whether or not a Part binds to it; it is checked here
    /// rather than in `add` because libraries load in order (built-ins, `libraries`,
    /// imports, the composition's own table) and an alias may name a behavior a later one
    /// declares. Resolving alone is not enough: `Registry::instrument` expands the behavior
    /// and returns its `output` untyped, so an alias to `output = {note = 200}` would
    /// survive a resolve-only check and a Part could then rescue it by overlaying its own
    /// `note`. The check is `output_table` rather than `instrument` because at this point the
    /// value IS the output: `instrument`'s alias branch would read `output = "anything"` as
    /// one more name to resolve and let a non-table through. The error names the file the
    /// alias was written in, the entry, and either the unresolved target or the bad value.
    fn check_aliases(&self) -> Result<(), String> {
        for (name, entry) in &self.kit {
            if let Value::String(_) = entry {
                let attach = |e: String| {
                    self.kit_origins
                        .get(name)
                        .map_or(e.clone(), |origin| origin.attach(e))
                };
                let resolved = self.instrument(name, &mut vec![]).map_err(attach)?;
                output_table(&resolved).map_err(|e| attach(format!("kit.{name}: {e}")))?;
            }
        }
        Ok(())
    }
    /// The output table an instrument name binds to. A string entry names a behavior and
    /// takes its `output` as-is, so an imported kit is read, never re-typed.
    fn instrument(&self, name: &str, stack: &mut Vec<String>) -> Result<Value, String> {
        let entry = self.kit.get(name).ok_or_else(|| {
            format!(
                "unknown instrument {name:?}; the kit declares {}",
                if self.kit.is_empty() {
                    "nothing".to_owned()
                } else {
                    self.kit
                        .keys()
                        .map(|k| format!("{k:?}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            )
        })?;
        match entry {
            Value::String(behavior) => {
                let definition = self
                    .behaviors
                    .get(behavior)
                    .ok_or_else(|| format!("kit.{name} names unknown behavior {behavior:?}"))?;
                if stack.len() >= 32 || stack.contains(behavior) {
                    return Err(format!(
                        "library dependency cycle or excessive depth: {} -> {behavior}",
                        stack.join(" -> ")
                    ));
                }
                stack.push(behavior.clone());
                let expanded = self.expand(definition, false, stack, &mut None)?;
                stack.pop();
                table(&expanded)?
                    .get("output")
                    .cloned()
                    .ok_or_else(|| format!("kit.{name}: behavior {behavior:?} has no output"))
            }
            output => Ok(output.clone()),
        }
    }
    fn expand(
        &self,
        value: &Value,
        profile: bool,
        stack: &mut Vec<String>,
        kit: &mut Option<String>,
    ) -> Result<Value, String> {
        let mut local = table(value)?.clone();
        if !profile {
            super::syntax::behavior(&mut local)?;
        }
        let used = local.remove("use");
        let composed = local.remove("compose");
        let bound = match local.remove("kit") {
            Some(_) if profile => return Err("kit binds a Part, not a profile".into()),
            Some(Value::String(name)) => Some(name),
            Some(_) => return Err("kit must name an instrument".into()),
            None => None,
        };
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
            let expanded = self.expand(definition, profile, stack, kit)?;
            stack.pop();
            merge(&mut result, expanded);
        }
        // The kit is the binding: it replaces any composed output outright, and only the
        // Part's own `output` fields may then overlay it (a gate, a note for a pitched voice).
        if let Some(name) = bound {
            let output = self.instrument(&name, stack)?;
            result
                .as_table_mut()
                .unwrap()
                .insert("output".into(), output);
            *kit = Some(name);
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
        registry.add(library, &Origin::File(path.clone()))?;
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

/// Expand a composition that must be complete: a kit gap is refused here.
pub fn expand(source: &str, base: Option<&Path>) -> Result<Value, String> {
    let draft = expand_with_libraries(source, base, &[])?;
    refuse(&draft.gaps)?;
    Ok(draft.value)
}

pub(super) fn expand_with_libraries(
    source: &str,
    base: Option<&Path>,
    libraries: &[PathBuf],
) -> Result<Draft, String> {
    let mut registry = Registry::default();
    for (path, source) in [
        (
            "library/drums/common.toml",
            include_str!("../../library/drums/common.toml"),
        ),
        (
            "library/drums/techno.toml",
            include_str!("../../library/drums/techno.toml"),
        ),
        (
            "library/drums/dnb.toml",
            include_str!("../../library/drums/dnb.toml"),
        ),
        (
            "library/accents/velocity.toml",
            include_str!("../../library/accents/velocity.toml"),
        ),
        (
            "library/accents/controls.toml",
            include_str!("../../library/accents/controls.toml"),
        ),
        (
            "library/kits/909.toml",
            include_str!("../../library/kits/909.toml"),
        ),
        (
            "library/grooves/drums.toml",
            include_str!("../../library/grooves/drums.toml"),
        ),
    ] {
        let origin = Origin::BuiltIn(path);
        let library = toml::from_str(source).map_err(|e| origin.attach(e.to_string()))?;
        registry.add(library, &origin)?;
    }
    let mut root: Table = toml::from_str(source).map_err(|e| e.to_string())?;
    let mut loaded = vec![];
    for path in libraries {
        load_library(path, &mut registry, &mut vec![], &mut loaded)?;
    }
    imports(&mut root, base, &mut registry, &mut vec![], &mut loaded)?;
    if let Some(library) = root.remove("library") {
        registry.add(library, &Origin::Composition)?;
    }
    // The registry is complete: every alias must now resolve, used or not (t-504).
    registry.check_aliases()?;
    // Every phrase expands the same Parts, so the same gap is found once per phrase.
    let gaps = RefCell::new(BTreeSet::new());
    let value = super::phrases::expand(root, &|root| expand_flat(&registry, root, &gaps))?;
    Ok(Draft {
        value,
        gaps: gaps.into_inner().into_iter().collect(),
    })
}
fn expand_flat(
    registry: &Registry,
    mut root: Table,
    gaps: &RefCell<BTreeSet<Gap>>,
) -> Result<Value, String> {
    super::syntax::keyed_parts(&mut root)?;
    if let Some(accents) = root.get_mut("accents") {
        let lanes = accents
            .as_table_mut()
            .ok_or("accents must be named tables")?;
        for (name, lane) in lanes {
            let fields = lane.as_table_mut().ok_or("shared accent must be a table")?;
            if let Some(rhythm) = fields.get_mut("rhythm") {
                super::syntax::rhythm(rhythm).map_err(|e| format!("accents.{name}: {e}"))?;
            }
        }
    }
    fn part(
        registry: &Registry,
        value: &Value,
        gaps: &RefCell<BTreeSet<Gap>>,
    ) -> Result<Value, String> {
        let mut kit = None;
        let mut expanded = registry.expand(value, false, &mut vec![], &mut kit)?;
        // Identity belongs to the composition, never a reusable definition.
        let id = table(value)?
            .get("id")
            .ok_or("every Part instance needs its own id")?
            .clone();
        let fields = expanded.as_table_mut().unwrap();
        fields.insert("id".into(), id.clone());
        if let Some(profile) = fields.get_mut("profile") {
            *profile = registry.expand(profile, true, &mut vec![], &mut None)?;
        }
        // An instrument read from the kit with no `controls` table has declared nothing:
        // the controls this Part addresses are listed as a gap and set aside, so the draft
        // still validates and inspects. An output that declares controls keeps the refusal
        // below for any name it does not list.
        if let Some(instrument) = kit
            && !fields
                .get("output")
                .and_then(Value::as_table)
                .is_some_and(|o| o.contains_key("controls"))
        {
            let mut unbound = vec![];
            if let Some(lanes) = fields.get_mut("parameters").and_then(Value::as_table_mut) {
                unbound.extend(lanes.keys().cloned());
                lanes.clear();
            }
            if let Some(controls) = fields
                .get_mut("profile")
                .and_then(Value::as_table_mut)
                .and_then(|p| p.get_mut("controls"))
                .and_then(Value::as_table_mut)
            {
                unbound.extend(controls.keys().cloned());
                controls.clear();
            }
            if !unbound.is_empty() {
                gaps.borrow_mut().insert(Gap {
                    part: id.as_str().unwrap_or("?").to_owned(),
                    instrument,
                    controls: unbound,
                });
            }
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
        *value =
            part(registry, value, gaps).map_err(|e| format!("parts.{}: {e}", fields_id(value)))?;
    }
    if let Some(value) = root.get_mut("parts") {
        let parts = value
            .as_array_mut()
            .ok_or("parts must be an array of tables")?;
        for value in parts {
            *value = part(registry, value, gaps)
                .map_err(|e| format!("parts.{}: {e}", fields_id(value)))?;
        }
    }
    Ok(Value::Table(root))
}
