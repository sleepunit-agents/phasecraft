//! Hardware preferences are local to this machine and keyed by canonical project path.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Routing {
    pub port: Option<String>,
    pub virtual_port: bool,
    pub silent: bool,
    pub send_clock: bool,
}
impl Routing {
    pub fn validate(&self) -> Result<(), String> {
        if self.port.as_ref().is_some_and(|p| p.trim().is_empty())
            || (self.port.is_some() as u8 + self.virtual_port as u8 + self.silent as u8) > 1
        {
            return Err("Choose one MIDI destination or silent preview".into());
        }
        if cfg!(target_os = "windows") && self.virtual_port {
            return Err("Select an existing MIDI port on Windows".into());
        }
        Ok(())
    }
}
#[derive(Default, Deserialize, Serialize)]
pub struct Settings {
    pub projects: BTreeMap<PathBuf, Routing>,
}
impl Settings {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&mut self, path: &Path, project: PathBuf, routing: Routing) -> Result<(), String> {
        routing.validate()?;
        let mut next = self.projects.clone();
        next.insert(project, routing);
        let data = serde_json::to_vec_pretty(&Self {
            projects: next.clone(),
        })
        .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(path.parent().ok_or("settings directory unavailable")?)
            .map_err(|e| e.to_string())?;
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&tmp, data)
            .and_then(|_| std::fs::rename(&tmp, path))
            .map_err(|e| e.to_string())?;
        self.projects = next;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routing_roundtrips_per_project_and_invalid_changes_preserve_it() {
        let dir = std::env::temp_dir().join(format!("phasecraft-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut settings = Settings::default();
        let route = Routing {
            port: Some("Phasecraft".into()),
            send_clock: true,
            ..Routing::default()
        };
        settings
            .save(&path, "project-a".into(), route.clone())
            .unwrap();
        settings
            .save(
                &path,
                "project-b".into(),
                Routing {
                    silent: true,
                    ..Routing::default()
                },
            )
            .unwrap();
        assert_eq!(
            Settings::load(&path).projects[Path::new("project-a")],
            route
        );
        assert!(
            settings
                .save(
                    &path,
                    "project-a".into(),
                    Routing {
                        silent: true,
                        ..route.clone()
                    }
                )
                .is_err()
        );
        assert_eq!(
            Settings::load(&path).projects[Path::new("project-a")],
            route
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
