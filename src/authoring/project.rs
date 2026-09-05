//! Project discovery and scaffolding; the musical engine never sees filesystem layout.
use crate::music::Composition;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    name: String,
    default: PathBuf,
    compositions: Vec<PathBuf>,
    #[serde(default)]
    libraries: Vec<PathBuf>,
    midi: PathBuf,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MidiSettings {
    pub port: Option<String>,
    pub virtual_port: bool,
    pub send_clock: bool,
    pub lookahead_ms: u64,
}
impl Default for MidiSettings {
    fn default() -> Self {
        Self {
            port: None,
            virtual_port: false,
            send_clock: false,
            lookahead_ms: 100,
        }
    }
}
pub struct Loaded {
    pub composition: Composition,
    pub midi: MidiSettings,
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&source).map_err(|e| format!("{}: {e}", path.display()))
}
fn manifest(path: &Path) -> Result<Manifest, String> {
    let m: Manifest = read(path)?;
    if m.name.trim().is_empty() || m.compositions.is_empty() || !m.compositions.contains(&m.default)
    {
        return Err(format!(
            "{}: name must be nonempty and compositions must include default",
            path.display()
        ));
    }
    for item in m
        .compositions
        .iter()
        .chain(m.libraries.iter())
        .chain([&m.default, &m.midi])
    {
        if item.as_os_str().is_empty()
            || item.is_absolute()
            || item.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::Prefix(_)
                )
            })
        {
            return Err(format!(
                "{}: project paths must be relative and stay inside the project: {}",
                path.display(),
                item.display()
            ));
        }
    }
    Ok(m)
}
fn locate(path: &Path) -> Result<(PathBuf, Option<PathBuf>), String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if path.is_dir() {
        return Ok((path.clone(), Some(path.join("phasecraft.toml"))));
    }
    if path.file_name().is_some_and(|s| s == "phasecraft.toml") {
        return Ok((path.clone(), Some(path)));
    }
    let project = path
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .map(|dir| dir.join("phasecraft.toml"))
        .find(|p| p.is_file());
    Ok((path, project))
}
pub fn load(path: &Path) -> Result<Loaded, String> {
    let (selected, project) = locate(path)?;
    let (file, libraries, midi) = if let Some(project) = project {
        let m = manifest(&project)?;
        let root = project.parent().unwrap();
        let file = if selected.is_dir() || selected == project {
            root.join(m.default)
        } else {
            selected
        };
        let midi_path = root.join(m.midi);
        let midi: MidiSettings = read(&midi_path)?;
        if !(10..=1000).contains(&midi.lookahead_ms)
            || (midi.virtual_port && midi.port.is_some())
            || midi.port.as_ref().is_some_and(|p| p.trim().is_empty())
        {
            return Err(format!(
                "{}: lookahead_ms must be 10..1000; choose a nonempty port or virtual_port, not both",
                midi_path.display()
            ));
        }
        (
            file,
            m.libraries.iter().map(|p| root.join(p)).collect::<Vec<_>>(),
            midi,
        )
    } else {
        (selected, vec![], MidiSettings::default())
    };
    let source = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let composition = super::library::expand_with_libraries(&source, file.parent(), &libraries)
        .and_then(|value| value.try_into().map_err(|e: toml::de::Error| e.to_string()))
        .map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(Loaded { composition, midi })
}

#[derive(Serialize)]
pub struct Validation {
    pub valid: bool,
    pub files: Vec<String>,
    pub errors: Vec<String>,
}
pub fn validate(path: &Path) -> Validation {
    let mut report = Validation {
        valid: true,
        files: vec![],
        errors: vec![],
    };
    let paths = (|| {
        let (selected, project) = locate(path)?;
        if let Some(project) = project.filter(|p| selected.is_dir() || selected == *p) {
            let m = manifest(&project)?;
            Ok(m.compositions
                .iter()
                .map(|p| project.parent().unwrap().join(p))
                .collect::<Vec<_>>())
        } else {
            Ok(vec![selected])
        }
    })();
    match paths {
        Err(e) => report.errors.push(e),
        Ok(paths) => {
            for path in paths {
                report.files.push(path.display().to_string());
                if let Err(e) = load(&path) {
                    report.errors.push(e);
                }
            }
        }
    }
    report.valid = report.errors.is_empty();
    report
}

const FILES: &[(&str, &str)] = &[
    (
        "phasecraft.toml",
        include_str!("../../templates/project/phasecraft.toml"),
    ),
    (
        "config/midi.toml",
        include_str!("../../templates/project/config/midi.toml"),
    ),
    (
        "compositions/techno.toml",
        include_str!("../../templates/project/compositions/techno.toml"),
    ),
    (
        "compositions/dnb.toml",
        include_str!("../../templates/project/compositions/dnb.toml"),
    ),
    (
        "compositions/garage.toml",
        include_str!("../../templates/project/compositions/garage.toml"),
    ),
    (
        "patterns/drums.toml",
        include_str!("../../templates/project/patterns/drums.toml"),
    ),
    (
        "patterns/accents.toml",
        include_str!("../../templates/project/patterns/accents.toml"),
    ),
    (
        "patterns/grooves.toml",
        include_str!("../../templates/project/patterns/grooves.toml"),
    ),
    (
        "kits/909.toml",
        include_str!("../../templates/project/kits/909.toml"),
    ),
    (
        "README.md",
        include_str!("../../templates/project/README.md"),
    ),
];
pub fn create(path: &Path) -> Result<(), String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.trim().is_empty())
        .ok_or("choose a new project directory name")?;
    // Refuse any existing destination, including an empty directory or symlink.
    fs::create_dir(path).map_err(|e| {
        format!(
            "cannot create {} (destination must be new): {e}",
            path.display()
        )
    })?;
    let result = (|| {
        for (relative, source) in FILES {
            let file = path.join(relative);
            fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
            let contents = if *relative == "phasecraft.toml" {
                let mut m: Manifest = toml::from_str(source).map_err(|e| e.to_string())?;
                m.name = name.into();
                toml::to_string_pretty(&m).map_err(|e| e.to_string())?
            } else {
                source.to_string()
            };
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&file)
                .and_then(|mut f| f.write_all(contents.as_bytes()))
                .map_err(|e| format!("{}: {e}", file.display()))?;
        }
        Ok(())
    })();
    result.map_err(|e: String| format!("project creation incomplete at {}: {e}", path.display()))
}

#[derive(Clone, Serialize)]
pub struct ProjectInfo {
    pub path: PathBuf,
    pub name: String,
    pub compositions: Vec<PathBuf>,
    pub default: PathBuf,
}
/// Describe a project without flattening its list into an arrangement.
pub fn describe(path: &Path) -> Result<ProjectInfo, String> {
    let (selected, project) = locate(path)?;
    if let Some(project) = project {
        let m = manifest(&project)?;
        let root = project.parent().unwrap();
        Ok(ProjectInfo {
            path: root.to_owned(),
            name: m.name,
            default: root.join(m.default),
            compositions: m.compositions.iter().map(|p| root.join(p)).collect(),
        })
    } else {
        Ok(ProjectInfo {
            name: selected
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            path: selected.clone(),
            default: selected.clone(),
            compositions: vec![selected],
        })
    }
}
