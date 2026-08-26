//! Persistent settings and the privacy behaviors they drive (P6).
//!
//! Stored as JSON in the platform config directory. The privacy toggles are the
//! point: auto-lock after inactivity, obscure the window when it loses focus,
//! and a no-trace mode that keeps the app from remembering which container was
//! last opened. Biometric / OS-native unlock (Touch ID, Windows Hello) needs
//! platform framework FFI and is deliberately deferred; see the plan.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

/// User settings. `#[serde(default)]` so an older or partial file still loads,
/// with any missing field taking its default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Auto-lock after this many minutes of inactivity. 0 disables it.
    pub auto_lock_minutes: u32,
    /// Blank the window contents while it is not the focused window.
    pub obscure_on_unfocus: bool,
    /// Remember nothing across sessions: no last-opened container.
    pub no_trace: bool,
    /// UI language code (see `i18n::Lang::code`). Defaults to English.
    pub language: String,
    /// Theme choice: "light", "dark", or "auto". Defaults to following the system.
    pub theme: String,
    /// Default cipher and hash the Create wizard starts on.
    pub default_scheme: String,
    pub default_prf: String,
    /// The last container opened, prefilled on next launch. Never written when
    /// `no_trace` is on.
    pub last_container: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 5,
            obscure_on_unfocus: true,
            no_trace: false,
            language: "en".to_owned(),
            theme: "auto".to_owned(),
            default_scheme: "AES".to_owned(),
            default_prf: "SHA-512".to_owned(),
            last_container: None,
        }
    }
}

impl Settings {
    /// The settings file path, or `None` if no config directory is available.
    pub fn path() -> Option<PathBuf> {
        ProjectDirs::from("se", "NorseHorse", "VaultPony")
            .map(|dirs| dirs.config_dir().join("settings.json"))
    }

    /// Load settings, falling back to defaults on any missing file or parse
    /// error (settings are best-effort, never load-bearing).
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist settings. No-trace is enforced here as well as at the call site:
    /// the remembered container is stripped before anything touches disk, so a
    /// no-trace session can never leave that trace even by mistake.
    pub fn save(&self) -> Result<()> {
        let Some(path) = Self::path() else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut to_save = self.clone();
        if to_save.no_trace {
            to_save.last_container = None;
        }
        let json = serde_json::to_vec_pretty(&to_save)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// The container to prefill on launch: the remembered one, unless no-trace.
    pub fn remembered_container(&self) -> Option<PathBuf> {
        if self.no_trace {
            None
        } else {
            self.last_container.clone()
        }
    }
}

/// Whether an idle, still-unlocked session should auto-lock now.
pub fn should_auto_lock(idle: Duration, timeout_minutes: u32, has_open_volume: bool) -> bool {
    timeout_minutes > 0
        && has_open_volume
        && idle >= Duration::from_secs(u64::from(timeout_minutes) * 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let s = Settings::default();
        assert_eq!(s.auto_lock_minutes, 5);
        assert!(s.obscure_on_unfocus);
        assert!(!s.no_trace);
    }

    #[test]
    fn json_round_trips() {
        let s = Settings {
            auto_lock_minutes: 12,
            no_trace: true,
            default_prf: "Whirlpool".to_owned(),
            ..Settings::default()
        };
        let bytes = serde_json::to_vec(&s).unwrap();
        let back: Settings = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn partial_json_fills_defaults() {
        let back: Settings = serde_json::from_str("{\"no_trace\":true}").unwrap();
        assert!(back.no_trace);
        assert_eq!(back.auto_lock_minutes, 5); // default filled in
        assert_eq!(back.default_scheme, "AES");
    }

    #[test]
    fn no_trace_hides_remembered_container() {
        let mut s = Settings {
            last_container: Some(PathBuf::from("/secret/vault.hc")),
            ..Settings::default()
        };
        assert!(s.remembered_container().is_some());
        s.no_trace = true;
        assert!(s.remembered_container().is_none());
    }

    #[test]
    fn auto_lock_threshold() {
        assert!(!should_auto_lock(Duration::from_secs(60), 0, true)); // disabled
        assert!(!should_auto_lock(Duration::from_secs(60), 5, true)); // not yet
        assert!(!should_auto_lock(Duration::from_secs(600), 5, false)); // nothing open
        assert!(should_auto_lock(Duration::from_secs(300), 5, true)); // exactly at
        assert!(should_auto_lock(Duration::from_secs(9999), 5, true)); // well past
    }
}
