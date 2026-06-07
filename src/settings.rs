use std::{collections::HashMap, fs, io, path::PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::ui;

const SETTINGS_FILE_NAME: &str = "settings.toml";
const APP_NAME: &str = "SwitchLayout";

const SHORTCUT_DEFINITIONS: &[ShortcutDefinition] = &[
    ShortcutDefinition {
        id: "switch_last_word",
        label: "Последнее слово",
        default_shortcut: "Ctrl + Alt + 1",
    },
    ShortcutDefinition {
        id: "switch_full_text",
        label: "Весь текст",
        default_shortcut: "Ctrl + Alt + 2",
    },
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AppSettings {
    pub(crate) autostart_enabled: bool,
    pub(crate) shortcuts: Vec<ShortcutBinding>,
}

impl AppSettings {
    pub(crate) fn to_shortcut_actions(&self) -> Vec<ui::ShortcutAction> {
        self.shortcuts
            .iter()
            .map(|binding| ui::ShortcutAction {
                label: shortcut_label(&binding.id).into(),
                shortcut: binding.shortcut.as_str().into(),
            })
            .collect()
    }

    pub(crate) fn set_shortcut(&mut self, index: usize, shortcut: impl Into<String>) -> bool {
        let Some(binding) = self.shortcuts.get_mut(index) else {
            return false;
        };

        binding.shortcut = shortcut.into();
        true
    }

    fn normalized(self) -> Self {
        let stored_shortcuts = self
            .shortcuts
            .into_iter()
            .map(|binding| (binding.id, binding.shortcut))
            .collect::<HashMap<_, _>>();

        let shortcuts = SHORTCUT_DEFINITIONS
            .iter()
            .map(|definition| ShortcutBinding {
                id: definition.id.to_string(),
                shortcut: stored_shortcuts
                    .get(definition.id)
                    .cloned()
                    .unwrap_or_else(|| definition.default_shortcut.to_string()),
            })
            .collect();

        Self { autostart_enabled: self.autostart_enabled, shortcuts }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            autostart_enabled: true,
            shortcuts: SHORTCUT_DEFINITIONS
                .iter()
                .map(|definition| ShortcutBinding {
                    id: definition.id.to_string(),
                    shortcut: definition.default_shortcut.to_string(),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ShortcutBinding {
    pub(crate) id: String,
    pub(crate) shortcut: String,
}

#[derive(Debug)]
pub(crate) struct SettingsStore {
    config_path: PathBuf,
}

impl SettingsStore {
    pub(crate) fn new() -> io::Result<Self> {
        let Some(project_dirs) = ProjectDirs::from("", "", APP_NAME) else {
            return Err(io::Error::other("failed to resolve the application config directory"));
        };

        Ok(Self::from_path(project_dirs.config_dir().join(SETTINGS_FILE_NAME)))
    }

    pub(crate) fn load_or_initialize(&self) -> io::Result<AppSettings> {
        match fs::read_to_string(&self.config_path) {
            Ok(contents) => match toml::from_str::<AppSettings>(&contents) {
                Ok(settings) => {
                    let normalized = settings.clone().normalized();

                    if normalized != settings {
                        self.save(&normalized)?;
                    }

                    Ok(normalized)
                }
                Err(_) => {
                    let defaults = AppSettings::default();
                    self.save(&defaults)?;
                    Ok(defaults)
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let defaults = AppSettings::default();
                self.save(&defaults)?;
                Ok(defaults)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn save(&self, settings: &AppSettings) -> io::Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let serialized = toml::to_string_pretty(settings)
            .map_err(|error| io::Error::other(format!("failed to serialize settings: {error}")))?;
        let temporary_path = self.temporary_path();

        fs::write(&temporary_path, serialized)?;
        replace_file(&temporary_path, &self.config_path)?;

        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn config_path(&self) -> &std::path::Path {
        &self.config_path
    }

    pub(crate) fn from_path(config_path: PathBuf) -> Self {
        Self { config_path }
    }

    fn temporary_path(&self) -> PathBuf {
        self.config_path.with_extension("toml.tmp")
    }
}

#[derive(Clone, Copy, Debug)]
struct ShortcutDefinition {
    id: &'static str,
    label: &'static str,
    default_shortcut: &'static str,
}

fn shortcut_label(id: &str) -> &'static str {
    SHORTCUT_DEFINITIONS
        .iter()
        .find(|definition| definition.id == id)
        .map(|definition| definition.label)
        .unwrap_or("Неизвестное действие")
}

fn replace_file(source: &std::path::Path, destination: &std::path::Path) -> io::Result<()> {
    if destination.exists() {
        fs::remove_file(destination)?;
    }

    fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn serializes_and_deserializes_settings() {
        let settings = AppSettings::default();

        let serialized = toml::to_string_pretty(&settings).expect("settings should serialize");
        let parsed = toml::from_str::<AppSettings>(&serialized).expect("settings should parse");

        assert_eq!(parsed, settings);
    }

    #[test]
    fn creates_defaults_when_file_is_missing() {
        let store = test_store("missing");

        let loaded =
            store.load_or_initialize().expect("missing settings file should be initialized");

        assert_eq!(loaded, AppSettings::default());
        assert!(store.config_path().exists());
    }

    #[test]
    fn replaces_corrupted_file_with_defaults() {
        let store = test_store("corrupted");
        let config_dir =
            store.config_path().parent().expect("config path should have a parent directory");

        fs::create_dir_all(config_dir).expect("config dir should be created");
        fs::write(store.config_path(), "broken = [").expect("corrupted file should be written");

        let loaded =
            store.load_or_initialize().expect("corrupted settings file should be replaced");
        let persisted =
            fs::read_to_string(store.config_path()).expect("settings file should exist");
        let reparsed =
            toml::from_str::<AppSettings>(&persisted).expect("replacement settings should parse");

        assert_eq!(loaded, AppSettings::default());
        assert_eq!(reparsed, AppSettings::default());
    }

    #[test]
    fn saves_and_reloads_settings() {
        let store = test_store("save_reload");
        let mut settings = AppSettings::default();
        settings.autostart_enabled = false;
        assert!(settings.set_shortcut(0, "Alt + Shift + K"));

        store.save(&settings).expect("settings should save");

        let reloaded = store.load_or_initialize().expect("saved settings should be reloaded");

        assert_eq!(reloaded, settings);
    }

    #[test]
    fn overwrites_existing_file_on_save() {
        let store = test_store("overwrite_existing");
        let initial = AppSettings::default();
        let mut updated = AppSettings::default();
        updated.autostart_enabled = false;
        assert!(updated.set_shortcut(1, "Ctrl + Shift + U"));

        store.save(&initial).expect("initial settings should save");
        store.save(&updated).expect("existing settings file should be replaced on save");

        let reloaded = store
            .load_or_initialize()
            .expect("updated settings should be readable after overwrite");
        assert_eq!(reloaded, updated);
    }

    fn test_store(test_name: &str) -> SettingsStore {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let config_path = std::env::temp_dir()
            .join(format!("switch_layout_{test_name}_{}_{}", std::process::id(), unique))
            .join(SETTINGS_FILE_NAME);

        SettingsStore::from_path(config_path)
    }
}
