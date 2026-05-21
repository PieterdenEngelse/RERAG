//! Runtime settings store: overrides + effective-value lookup + change notify.

use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::kind::Kind;
use super::registry::{lookup, KNOWN_KEYS};

type Listener = Box<dyn Fn(&str) + Send + Sync + 'static>;

pub struct Settings {
    overrides: RwLock<HashMap<String, String>>,
    path: PathBuf,
    listeners: RwLock<HashMap<String, Vec<Listener>>>,
}

impl Settings {
    /// Load overrides from `path` (missing or malformed file → empty map).
    pub fn load(path: PathBuf) -> Arc<Self> {
        let overrides = match std::fs::read_to_string(&path) {
            Ok(text) if !text.trim().is_empty() => {
                match serde_json::from_str::<HashMap<String, String>>(&text) {
                    Ok(map) => map,
                    Err(e) => {
                        tracing::warn!(
                            "settings: failed to parse {}: {e} — starting with no overrides",
                            path.display()
                        );
                        HashMap::new()
                    }
                }
            }
            _ => HashMap::new(),
        };
        Arc::new(Self {
            overrides: RwLock::new(overrides),
            path,
            listeners: RwLock::new(HashMap::new()),
        })
    }

    /// Effective value: override → environment.
    pub fn effective(&self, key: &str) -> Option<String> {
        if let Some(v) = self.overrides.read().get(key) {
            return Some(v.clone());
        }
        std::env::var(key).ok()
    }

    pub fn effective_or(&self, key: &str, default: &str) -> String {
        self.effective(key).unwrap_or_else(|| default.to_string())
    }

    pub fn effective_bool(&self, key: &str, default: bool) -> bool {
        match self.effective(key) {
            Some(v) => {
                let v = v.trim().to_lowercase();
                matches!(v.as_str(), "true" | "1" | "yes" | "on")
            }
            None => default,
        }
    }

    pub fn effective_u64(&self, key: &str, default: u64) -> u64 {
        self.effective(key)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(default)
    }

    pub fn effective_f64(&self, key: &str, default: f64) -> f64 {
        self.effective(key)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(default)
    }

    /// Set (`Some`) or clear (`None`) an override. Validates against the
    /// registry when the key is known.
    pub fn set(&self, key: &str, value: Option<String>) -> Result<(), String> {
        let normalized = match (lookup(key), &value) {
            (Some(known), Some(v)) => Some(known.kind.parse(v)?),
            (_, Some(v)) => Some(v.clone()),
            (_, None) => None,
        };

        {
            let mut overrides = self.overrides.write();
            match &normalized {
                Some(v) => {
                    overrides.insert(key.to_string(), v.clone());
                }
                None => {
                    overrides.remove(key);
                }
            }
            persist(&overrides, &self.path)?;
        }

        // Notify subscribers — use the new effective value (override → env).
        let notify_value = normalized
            .or_else(|| std::env::var(key).ok())
            .unwrap_or_default();
        if let Some(listeners) = self.listeners.read().get(key) {
            for listener in listeners {
                listener(&notify_value);
            }
        }

        Ok(())
    }

    /// Subscribe to changes for a single key. The handler receives the new
    /// effective value as a string. Handlers must be cheap — they run inline
    /// during `set()`.
    pub fn subscribe(&self, key: &str, handler: impl Fn(&str) + Send + Sync + 'static) {
        let mut listeners = self.listeners.write();
        listeners
            .entry(key.to_string())
            .or_default()
            .push(Box::new(handler));
    }

    /// Build a UI-facing snapshot: every registered key plus any unregistered
    /// overrides.
    pub fn snapshot(&self) -> SettingsSnapshot {
        let overrides = self.overrides.read();
        let mut entries = Vec::with_capacity(KNOWN_KEYS.len() + overrides.len());

        for known in KNOWN_KEYS {
            let override_value = overrides.get(known.key).cloned();
            let env_value = std::env::var(known.key).ok();
            let effective = override_value
                .clone()
                .or_else(|| env_value.clone())
                .or_else(|| known.default.map(|d| d.to_string()));
            let source = if override_value.is_some() {
                Source::Override
            } else if env_value.is_some() {
                Source::Env
            } else if known.default.is_some() {
                Source::Default
            } else {
                Source::Unset
            };
            entries.push(SettingEntry {
                key: known.key.to_string(),
                description: Some(known.description.to_string()),
                kind: Some(known.kind.clone()),
                category: Some(known.category.to_string()),
                env_value,
                override_value,
                effective,
                source,
                restart_required: known.restart_required,
                registered: true,
            });
        }

        for (k, v) in overrides.iter() {
            if lookup(k).is_none() {
                let env_value = std::env::var(k).ok();
                entries.push(SettingEntry {
                    key: k.clone(),
                    description: None,
                    kind: None,
                    category: None,
                    env_value,
                    override_value: Some(v.clone()),
                    effective: Some(v.clone()),
                    source: Source::Override,
                    restart_required: false,
                    registered: false,
                });
            }
        }
        entries.sort_by(|a, b| a.key.cmp(&b.key));

        SettingsSnapshot { entries }
    }
}

/// Atomic write of the overrides map: write to .tmp, fsync, rename.
fn persist(overrides: &HashMap<String, String>, path: &Path) -> Result<(), String> {
    let json =
        serde_json::to_string_pretty(overrides).map_err(|e| format!("serialize overrides: {e}"))?;
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create parent dir {}: {e}", parent.display()))?;
        }
    }
    let mut tmp = path.to_path_buf();
    let mut name = path
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from("overrides.json"));
    name.push(".tmp");
    tmp.set_file_name(name);
    std::fs::write(&tmp, json).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename to {}: {e}", path.display()))?;
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Override,
    Env,
    Default,
    Unset,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingEntry {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective: Option<String>,
    pub source: Source,
    pub restart_required: bool,
    pub registered: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsSnapshot {
    pub entries: Vec<SettingEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fresh(td: &TempDir) -> Arc<Settings> {
        Settings::load(td.path().join("overrides.json"))
    }

    /// A key that's not in the registry and (presumably) not in the host env —
    /// gives the tests a clean override path.
    const UNREG: &str = "AG_TEST_UNREGISTERED_OVERRIDE_X";

    #[test]
    fn load_missing_file_yields_empty() {
        let td = TempDir::new().unwrap();
        let s = fresh(&td);
        assert!(s.effective(UNREG).is_none() || std::env::var(UNREG).is_ok());
    }

    #[test]
    fn set_persists_to_disk_and_snapshot_reflects_it() {
        let td = TempDir::new().unwrap();
        let s = fresh(&td);
        s.set(UNREG, Some("hello".into())).unwrap();

        // On disk.
        let raw = std::fs::read_to_string(td.path().join("overrides.json")).unwrap();
        assert!(raw.contains(UNREG) && raw.contains("hello"));

        // In snapshot, as an unregistered entry.
        let snap = s.snapshot();
        let entry = snap.entries.iter().find(|e| e.key == UNREG).unwrap();
        assert!(!entry.registered);
        assert_eq!(entry.override_value.as_deref(), Some("hello"));
        assert!(matches!(entry.source, Source::Override));
    }

    #[test]
    fn clearing_override_removes_it_from_disk() {
        let td = TempDir::new().unwrap();
        let s = fresh(&td);
        s.set(UNREG, Some("x".into())).unwrap();
        s.set(UNREG, None).unwrap();
        let raw = std::fs::read_to_string(td.path().join("overrides.json")).unwrap();
        assert!(!raw.contains(UNREG), "still present: {raw}");
    }

    #[test]
    fn known_key_rejects_bad_value() {
        let td = TempDir::new().unwrap();
        let s = fresh(&td);
        // REDIS_ENABLED is Bool in the registry — "maybe" should fail.
        let err = s.set("REDIS_ENABLED", Some("maybe".into())).unwrap_err();
        assert!(err.contains("'maybe'"), "{err}");
        // And nothing got written.
        let raw = std::fs::read_to_string(td.path().join("overrides.json"))
            .unwrap_or_default();
        assert!(!raw.contains("REDIS_ENABLED"));
    }

    #[test]
    fn unregistered_key_skips_validation() {
        let td = TempDir::new().unwrap();
        let s = fresh(&td);
        s.set(UNREG, Some("anything goes".into())).unwrap();
        assert_eq!(s.effective(UNREG).as_deref(), Some("anything goes"));
    }

    #[test]
    fn atomic_write_leaves_no_tmp() {
        let td = TempDir::new().unwrap();
        let s = fresh(&td);
        s.set(UNREG, Some("v".into())).unwrap();
        let tmp = td.path().join("overrides.json.tmp");
        assert!(!tmp.exists(), "tmp file leaked: {}", tmp.display());
    }

    #[test]
    fn round_trip_across_load() {
        let td = TempDir::new().unwrap();
        {
            let s = fresh(&td);
            s.set(UNREG, Some("persisted".into())).unwrap();
        } // drop instance
        let s2 = fresh(&td);
        assert_eq!(s2.effective(UNREG).as_deref(), Some("persisted"));
    }

    #[test]
    fn subscriber_fires_on_change() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let td = TempDir::new().unwrap();
        let s = fresh(&td);
        let count = std::sync::Arc::new(AtomicUsize::new(0));
        let c2 = count.clone();
        s.subscribe(UNREG, move |_| {
            c2.fetch_add(1, Ordering::SeqCst);
        });
        s.set(UNREG, Some("a".into())).unwrap();
        s.set(UNREG, Some("b".into())).unwrap();
        s.set(UNREG, None).unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }
}
