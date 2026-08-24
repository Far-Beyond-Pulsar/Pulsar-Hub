use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const OCCLUSION_ENV: &str = "WGPUI_OCCLUSION";

/// Name of the flag file stored beside the installed engine.
pub const FILE_NAME: &str = "launch-flags.toml";

/// A launch flag the Hub knows how to edit for an installed engine version.
///
/// The engine reads each variable once at startup (`LazyLock`), so values are
/// applied to the spawned process' environment rather than changed at runtime.
pub struct KnownFlag {
    /// The environment variable the engine reads.
    pub env: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// Mirrors the engine's own default so the panel matches engine behavior
    /// until this install has a flag file.
    pub default_on: bool,
}

pub const KNOWN_FLAGS: &[KnownFlag] = &[
    KnownFlag {
        env: "WGPUI_LAYERS",
        label: "Layer compositing",
        description: "Cache element subtrees as retained layers and composite them.",
        default_on: true,
    },
    KnownFlag {
        env: "WGPUI_INSTANCES",
        label: "Element instances",
        description: "Retain elements across frames so unchanged subtrees skip work.",
        default_on: true,
    },
    KnownFlag {
        env: "WGPUI_PERSISTENT_LAYOUT",
        label: "Persistent layout",
        description: "Keep layout trees between frames instead of relaying out every frame.",
        default_on: true,
    },
    KnownFlag {
        env: "WGPUI_SLABS",
        label: "Slab rendering",
        description: "Batch draw calls through packed GPU slab scenes.",
        default_on: true,
    },
    KnownFlag {
        env: "WGPUI_SLAB_COMPACTION",
        label: "Slab compaction",
        description: "Compact GPU slabs as scenes change to reduce memory waste.",
        default_on: true,
    },
    KnownFlag {
        env: OCCLUSION_ENV,
        label: "Occlusion culling",
        description: "Skip drawing elements fully covered by opaque regions.",
        default_on: true,
    },
    KnownFlag {
        env: "WGPUI_RENDER_STATS",
        label: "Render stats",
        description: "Dump accumulated renderer timings once per second.",
        default_on: false,
    },
    KnownFlag {
        env: "WGPUI_LAYER_DEBUG",
        label: "Layer debug tint",
        description: "Tint every composited layer to visualize layer caching.",
        default_on: false,
    },
];

const HEADER_COMMENT: &str = "# Pulsar Hub launch flags.
#
# This file is rewritten by Pulsar Hub whenever launch flags are edited for
# this engine version in the Versions screen, and it is applied as process
# environment variables every time the Hub launches that version.
# Entries under [flags] that Pulsar Hub does not recognize are preserved.

";

/// Per-version launch flags: which engine feature toggles the Hub sets in the
/// spawned engine's environment.
///
/// Flags absent from [`Self::overrides`] fall back to the engine default via
/// [`KnownFlag::default_on`], both in the UI ([`Self::checked`]) and on disk:
/// `save` writes only what differs from nothing, and `load` merges the file on
/// top of defaults so hand-trimmed files keep working.
#[derive(Debug, Clone, Default)]
pub struct LaunchFlags {
    overrides: BTreeMap<&'static str, bool>,
    /// Unknown `[flags]` entries from an existing file, rewritten verbatim so
    /// hand edits or future Hub fields survive a rewrite.
    extras: BTreeMap<String, toml::Value>,
}

impl LaunchFlags {
    pub fn file_path(dir: &Path) -> PathBuf {
        dir.join(FILE_NAME)
    }

    /// Load flags for the install in `dir`. A missing or corrupt file yields
    /// plain engine defaults; a corrupt one additionally logs a warning.
    pub fn load(dir: &Path) -> Self {
        let path = Self::file_path(dir);
        let mut flags = Self::default();
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(_) => return flags,
        };
        let document = match contents.parse::<toml::Table>() {
            Ok(document) => document,
            Err(error) => {
                tracing::warn!("Ignoring corrupt {}: {}", path.display(), error);
                return flags;
            }
        };
        match document.get("flags") {
            Some(toml::Value::Table(entries)) => {
                for (key, value) in entries {
                    if let Some(flag) = KNOWN_FLAGS.iter().find(|f| f.env == key.as_str()) {
                        match value.as_bool() {
                            Some(on) => {
                                flags.overrides.insert(flag.env, on);
                            }
                            None => tracing::warn!(
                                "{}: {} must be true/false; using the engine default",
                                path.display(),
                                key
                            ),
                        }
                    } else {
                        flags.extras.insert(key.clone(), value.clone());
                    }
                }
            }
            Some(_) => {
                tracing::warn!(
                    "{}: [flags] is not a table; using engine defaults",
                    path.display()
                )
            }
            None => {}
        }
        flags
    }

    /// Write the flag file for the install in `dir`. Called on first toggle so
    /// installs never touched in the Versions screen stay pristine.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let body =
            toml::to_string(&self.to_toml_document()).map_err(|error| error.to_string())?;
        std::fs::write(Self::file_path(dir), format!("{}{}", HEADER_COMMENT, body))
            .map_err(|error| error.to_string())
    }

    /// The effective value shown for `flag`: its saved override, else the
    /// engine default.
    pub fn checked(&self, flag: &KnownFlag) -> bool {
        self.overrides
            .get(flag.env)
            .copied()
            .unwrap_or(flag.default_on)
    }

    pub fn set(&mut self, env: &'static str, on: bool) {
        self.overrides.insert(env, on);
    }

    /// Drop every override so [`Self::checked`] reports engine defaults again.
    /// Unknown `[flags]` entries are kept and rewritten on the next save.
    pub fn reset_to_defaults(&mut self) {
        self.overrides.clear();
    }

    /// Apply this version's flags onto a command about to spawn the engine.
    ///
    /// Every flag sets `<ENV>=1` when on and `<ENV>=0` when off, except
    /// `WGPUI_OCCLUSION`: leaving it unset selects the engine's Normal mode,
    /// while the engine also supports `validate`, which the boolean toggle
    /// cannot express - so "on" omits the variable instead of writing "1".
    pub fn apply_to_command(&self, command: &mut Command) {
        for flag in KNOWN_FLAGS {
            let Some(on) = self.overrides.get(flag.env).copied() else {
                continue;
            };
            if flag.env == OCCLUSION_ENV && on {
                continue;
            }
            command.env(flag.env, if on { "1" } else { "0" });
        }
    }

    /// Apply flags stored in `dir` onto a command about to spawn that install.
    /// Without a flag file this changes nothing, matching stock behavior.
    pub fn apply_env(dir: &Path, command: &mut Command) {
        if !Self::file_path(dir).exists() {
            return;
        }
        Self::load(dir).apply_to_command(command);
    }

    fn to_toml_document(&self) -> toml::Table {
        let mut flags_table = toml::Table::new();
        for (env, on) in &self.overrides {
            flags_table.insert((*env).to_string(), toml::Value::Boolean(*on));
        }
        for (key, value) in &self.extras {
            flags_table.insert(key.clone(), value.clone());
        }
        let mut document = toml::Table::new();
        document.insert("flags".to_string(), toml::Value::Table(flags_table));
        document
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn env_value(command: &Command, name: &str) -> Option<String> {
        command
            .get_envs()
            .find(|(key, _)| *key == OsStr::new(name))
            .and_then(|(_, value)| value.map(|value| value.to_string_lossy().into_owned()))
    }

    #[test]
    fn known_flag_defaults_mirror_the_engine() {
        let defaults: Vec<(&str, bool)> = KNOWN_FLAGS
            .iter()
            .map(|flag| (flag.env, flag.default_on))
            .collect();
        assert_eq!(
            defaults,
            vec![
                ("WGPUI_LAYERS", true),
                ("WGPUI_INSTANCES", true),
                ("WGPUI_PERSISTENT_LAYOUT", true),
                ("WGPUI_SLABS", true),
                ("WGPUI_SLAB_COMPACTION", true),
                ("WGPUI_OCCLUSION", true),
                ("WGPUI_RENDER_STATS", false),
                ("WGPUI_LAYER_DEBUG", false),
            ]
        );
    }

    #[test]
    fn round_trips_toggles_through_disk() {
        let dir = tempfile::tempdir().expect("create temp directory");
        let mut flags = LaunchFlags::load(dir.path());
        assert!(!dir.path().join(FILE_NAME).exists());

        flags.set("WGPUI_LAYERS", false);
        flags.set("WGPUI_RENDER_STATS", true);
        flags.save(dir.path()).expect("save flags");

        let reloaded = LaunchFlags::load(dir.path());
        let known = |env: &str| {
            KNOWN_FLAGS
                .iter()
                .find(|flag| flag.env == env)
                .expect("flag exists in KNOWN_FLAGS")
        };
        assert!(!reloaded.checked(known("WGPUI_LAYERS")));
        assert!(reloaded.checked(known("WGPUI_RENDER_STATS")));
    }

    #[test]
    fn missing_file_yields_engine_defaults() {
        let dir = tempfile::tempdir().expect("create temp directory");

        let flags = LaunchFlags::load(dir.path());
        for flag in KNOWN_FLAGS {
            assert_eq!(flags.checked(flag), flag.default_on, "{}", flag.env);
        }
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults_without_panicking() {
        let dir = tempfile::tempdir().expect("create temp directory");
        std::fs::write(dir.path().join(FILE_NAME), "[flags\nWGPUI_LAYERS ===")
            .expect("write corrupt file");

        let flags = LaunchFlags::load(dir.path());
        for flag in KNOWN_FLAGS {
            assert_eq!(flags.checked(flag), flag.default_on, "{}", flag.env);
        }
    }

    #[test]
    fn unknown_keys_are_preserved_across_rewrites() {
        let dir = tempfile::tempdir().expect("create temp directory");
        std::fs::write(
            dir.path().join(FILE_NAME),
            "# custom note\n\n[flags]\nWGPUI_FUTURE_FEATURE = \"on\"\nWGPUI_SLABS = false\n",
        )
        .expect("write flags file");

        let mut flags = LaunchFlags::load(dir.path());
        flags.set("WGPUI_LAYERS", false);
        flags.save(dir.path()).expect("rewrite flags file");

        let reloaded = LaunchFlags::load(dir.path());
        assert!(!reloaded.extras.contains_key("WGPUI_SLABS"));
        assert_eq!(
            reloaded.extras.get("WGPUI_FUTURE_FEATURE"),
            Some(&toml::Value::String("on".to_string()))
        );

        let document = std::fs::read_to_string(dir.path().join(FILE_NAME))
            .expect("read rewritten file")
            .parse::<toml::Table>()
            .expect("rewritten file parses");
        assert_eq!(
            document["flags"]["WGPUI_FUTURE_FEATURE"],
            toml::Value::String("on".to_string())
        );
    }

    #[test]
    fn applies_one_and_zero_with_occlusion_omitted_when_on() {
        let mut command = Command::new("pulsar");
        let mut flags = LaunchFlags::default();
        flags.set("WGPUI_RENDER_STATS", true);
        flags.set("WGPUI_OCCLUSION", true);
        flags.set("WGPUI_LAYERS", false);
        flags.apply_to_command(&mut command);

        assert_eq!(
            env_value(&command, "WGPUI_RENDER_STATS").as_deref(),
            Some("1")
        );
        assert_eq!(env_value(&command, "WGPUI_LAYERS").as_deref(), Some("0"));
        assert_eq!(
            env_value(&command, "WGPUI_OCCLUSION"),
            None,
            "occlusion on stays unset so the engine keeps its Normal mode"
        );
    }

    #[test]
    fn occlusion_off_maps_to_zero() {
        let mut command = Command::new("pulsar");
        let mut flags = LaunchFlags::default();
        flags.set("WGPUI_OCCLUSION", false);
        flags.apply_to_command(&mut command);

        assert_eq!(env_value(&command, "WGPUI_OCCLUSION").as_deref(), Some("0"));
    }

    #[test]
    fn absent_file_changes_no_environment() {
        let dir = tempfile::tempdir().expect("create temp directory");
        let mut command = Command::new("pulsar");

        LaunchFlags::apply_env(dir.path(), &mut command);

        assert_eq!(command.get_envs().count(), 0);
    }
}
