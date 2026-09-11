use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::PathBuf;

pub const LOCAL_LIST_ID: &str = "local://default";

fn default_provider() -> String {
    "generic".to_string()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    /// Stable list href selected in the UI. Legacy configurations omit this;
    /// callers then fall back to the configured calendar name until the next save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_list: Option<String>,
    #[serde(default)]
    pub ui: UiConfig,
    /// Persisted list filters. The list is narrowed by these, not by which
    /// collection is selected, so a filter survives a restart.
    #[serde(default)]
    pub view: ViewConfig,
    /// `nextcloud` is accepted as a read-only compatibility alias for v1.0.x.
    #[serde(alias = "nextcloud")]
    pub caldav: CaldavConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            active_list: Some(LOCAL_LIST_ID.to_string()),
            ui: UiConfig::default(),
            view: ViewConfig::default(),
            caldav: CaldavConfig::default(),
        }
    }
}

/// Filters applied to the task list, persisted so a narrowed list survives a
/// restart. The UI shows an indicator whenever [`ViewConfig::is_filtered`] is
/// true, so an unexpectedly short list is always explainable.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ViewConfig {
    /// Widens the list to include finished tasks. Not a narrowing filter.
    #[serde(default)]
    pub include_completed: bool,
    /// Due-date preset: `""` (no narrowing), `overdue`, `today`, `has`, `none`.
    #[serde(default)]
    pub due: String,
    /// An exact all-day date token (`YYYYMMDD`), set by tapping a due pill.
    /// Mutually exclusive with `due`; the setter clears whichever is not chosen.
    #[serde(default)]
    pub due_on: String,
    /// Exact tags to narrow by, e.g. `project:kitchen`. A task matches if it
    /// carries any of them.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Tag namespaces to narrow by, e.g. `project` selects every `project:*`.
    /// Matched against each tag's key, so it is the broader of the two.
    #[serde(default)]
    pub tag_keys: Vec<String>,
}

impl ViewConfig {
    /// Whether anything is hiding tasks that would otherwise be listed.
    ///
    /// `include_completed` is excluded on purpose: it only ever makes the list
    /// longer, and warning someone that they can see more would be noise.
    pub fn is_filtered(&self) -> bool {
        !self.due.is_empty()
            || !self.due_on.is_empty()
            || !self.tags.is_empty()
            || !self.tag_keys.is_empty()
    }
}

fn default_color_mode() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiConfig {
    /// `auto` | `on` | `off`. `auto` colors pills only on a display that can
    /// render color; the other two override the probe in either direction, for
    /// a device the probe doesn't recognize.
    #[serde(default = "default_color_mode")]
    pub color: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            color: default_color_mode(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CaldavConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    pub base_url: String,
    pub username: String,
    pub app_password: String,
    /// Stable collection URL. New configurations use this for selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_href: Option<String>,
    /// Human-facing collection name. `calendar` is retained on disk for
    /// compatibility with v1.0.x and is no longer treated as an identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar: Option<String>,
}

pub type NextcloudConfig = CaldavConfig;

impl Default for CaldavConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            base_url: String::new(),
            username: String::new(),
            app_password: String::new(),
            calendar_href: None,
            calendar: None,
        }
    }
}

pub fn path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("could not resolve user config dir")?;
    Ok(base.join("retaskable").join("config.toml"))
}

pub fn load() -> Result<Config> {
    let path = path()?;
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Ok(Config::default());
        }
        Err(e) => {
            return Err(
                anyhow::Error::new(e).context(format!("reading config at {}", path.display()))
            );
        }
    };
    toml::from_str(&raw).with_context(|| format!("parsing config at {}", path.display()))
}

/// Like [`load`], but returns `Ok(None)` when no config file exists yet.
/// The Settings screen and local-only mode both tolerate a missing file.
pub fn load_optional() -> Result<Option<Config>> {
    let path = path()?;
    match std::fs::read_to_string(&path) {
        Ok(raw) => {
            let cfg: Config = toml::from_str(&raw)
                .with_context(|| format!("parsing config at {}", path.display()))?;
            Ok(Some(cfg))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => {
            Err(anyhow::Error::new(e).context(format!("reading config at {}", path.display())))
        }
    }
}

/// Resolve the app password to persist for a save: a blank new value keeps the
/// previously-stored one. The UI never round-trips the secret, so an unchanged
/// password arrives as an empty string.
pub fn merge_password(new_password: &str, existing: Option<&str>) -> String {
    if new_password.is_empty() {
        existing.unwrap_or("").to_string()
    } else {
        new_password.to_string()
    }
}

/// Serialize and write `cfg` to `path`, creating parent dirs. Overwrites any
/// existing file (unlike the create-new template writer).
pub fn save_to(path: &std::path::Path, cfg: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating config dir {}", parent.display()))?;
    }
    let body = toml::to_string(cfg).context("serializing config to TOML")?;
    std::fs::write(path, body).with_context(|| format!("writing config to {}", path.display()))
}

/// Write `cfg` to the standard config location.
pub fn save(cfg: &Config) -> Result<()> {
    save_to(&path()?, cfg)
}

pub fn active_list(cfg: &Config) -> Option<&str> {
    cfg.active_list
        .as_deref()
        .or(cfg.caldav.calendar_href.as_deref())
}

pub fn has_remote(cfg: &Config) -> bool {
    !cfg.caldav.base_url.trim().is_empty()
        && !cfg.caldav.username.trim().is_empty()
        && !cfg.caldav.app_password.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_password_blank_keeps_existing() {
        assert_eq!(merge_password("", Some("old-secret")), "old-secret");
        assert_eq!(merge_password("", None), "");
        assert_eq!(
            merge_password("new-secret", Some("old-secret")),
            "new-secret"
        );
    }

    #[test]
    fn save_to_then_parse_roundtrips_all_fields() {
        let path = std::env::temp_dir().join("retaskable_cfg_roundtrip_test.toml");
        let _ = std::fs::remove_file(&path);
        let cfg = Config {
            active_list: Some("https://nc.example.com/tasks/".into()),
            caldav: CaldavConfig {
                provider: "generic".into(),
                base_url: "https://nc.example.com".to_string(),
                username: "alice".to_string(),
                app_password: "abc-def-ghi".to_string(),
                calendar_href: Some("https://nc.example.com/tasks/".into()),
                calendar: Some("Personal".to_string()),
            },
            ..Default::default()
        };
        save_to(&path, &cfg).expect("save");
        let parsed: Config =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).expect("parse");
        assert_eq!(parsed.caldav.base_url, "https://nc.example.com");
        assert_eq!(parsed.caldav.username, "alice");
        assert_eq!(parsed.caldav.app_password, "abc-def-ghi");
        assert_eq!(parsed.caldav.calendar.as_deref(), Some("Personal"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_to_omits_calendar_when_none_and_still_parses() {
        let path = std::env::temp_dir().join("retaskable_cfg_none_test.toml");
        let _ = std::fs::remove_file(&path);
        let cfg = Config {
            active_list: Some(LOCAL_LIST_ID.into()),
            caldav: CaldavConfig {
                provider: "generic".into(),
                base_url: "https://x".into(),
                username: "u".into(),
                app_password: "p".into(),
                calendar_href: None,
                calendar: None,
            },
            ..Default::default()
        };
        save_to(&path, &cfg).expect("save");
        let parsed: Config =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).expect("parse");
        assert!(parsed.caldav.calendar.is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_nextcloud_table_parses() {
        let parsed: Config = toml::from_str(
            r#"[nextcloud]
base_url = "https://example.test"
username = "u"
app_password = "p"
calendar = "Tasks"
"#,
        )
        .unwrap();
        assert_eq!(parsed.caldav.base_url, "https://example.test");
        assert_eq!(parsed.caldav.provider, "generic");
        assert_eq!(parsed.active_list, None);
    }

    #[test]
    fn missing_config_defaults_to_local_capable_shape() {
        let parsed = Config::default();
        assert_eq!(active_list(&parsed), Some(LOCAL_LIST_ID));
        assert_eq!(parsed.caldav.provider, "generic");
        assert!(!has_remote(&parsed));
        assert_eq!(parsed.ui.color, "auto");
    }
}
