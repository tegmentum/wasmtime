//! Host bundle and manifest support for Wasmtime CLI.
//!
//! This module provides support for loading host implementations via bundles
//! and manifest files. Host bundles package WIT definitions and native
//! implementations together, while manifest files allow configuring multiple
//! hosts from a single configuration file.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Configuration for a host bundle as defined in host.toml
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HostBundleConfig {
    #[serde(flatten)]
    pub host: HostConfig,
}

/// Host configuration within a bundle
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HostConfig {
    /// Name of the host
    pub name: String,

    /// Path to the native library (relative to bundle root)
    pub lib: PathBuf,

    /// Path to the WIT directory or file (relative to bundle root)
    pub wit: PathBuf,
}

/// A host bundle containing WIT definitions and native implementation
#[derive(Debug, Clone)]
pub struct HostBundle {
    /// The bundle configuration
    pub config: HostBundleConfig,

    /// Absolute path to the bundle root directory
    pub bundle_path: PathBuf,
}

impl HostBundle {
    /// Load a host bundle from a directory
    pub fn load_from_dir(bundle_path: impl AsRef<Path>) -> Result<Self> {
        let bundle_path = bundle_path.as_ref();

        if !bundle_path.is_dir() {
            bail!("Host bundle path is not a directory: {}", bundle_path.display());
        }

        let config_path = bundle_path.join("host.toml");
        if !config_path.exists() {
            bail!("Host bundle missing host.toml: {}", bundle_path.display());
        }

        let config_content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read host.toml from {}", config_path.display()))?;

        let config: HostBundleConfig = toml::from_str(&config_content)
            .with_context(|| format!("Failed to parse host.toml from {}", config_path.display()))?;

        // Validate that the referenced files exist
        let lib_path = bundle_path.join(&config.host.lib);
        if !lib_path.exists() {
            bail!(
                "Host bundle lib not found: {} (referenced in {})",
                lib_path.display(),
                config_path.display()
            );
        }

        let wit_path = bundle_path.join(&config.host.wit);
        if !wit_path.exists() {
            bail!(
                "Host bundle WIT not found: {} (referenced in {})",
                wit_path.display(),
                config_path.display()
            );
        }

        Ok(Self {
            config,
            bundle_path: bundle_path.to_path_buf(),
        })
    }

    /// Get the absolute path to the native library
    pub fn lib_path(&self) -> PathBuf {
        self.bundle_path.join(&self.config.host.lib)
    }

    /// Get the absolute path to the WIT directory/file
    pub fn wit_path(&self) -> PathBuf {
        self.bundle_path.join(&self.config.host.wit)
    }

    /// Get the host name
    pub fn name(&self) -> &str {
        &self.config.host.name
    }
}

/// Global configuration for host manifests
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GlobalConfig {
    /// Search paths for host bundles
    #[serde(default)]
    pub search_paths: Vec<PathBuf>,
}

/// A host entry in the manifest, can reference a bundle or provide explicit paths
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum HostEntry {
    /// Reference to a bundle by name
    Bundle {
        name: String,
        bundle: String,
    },
    /// Explicit WIT and lib paths
    Explicit {
        name: String,
        wit: PathBuf,
        lib: PathBuf,
    },
}

impl HostEntry {
    /// Get the host name
    pub fn name(&self) -> &str {
        match self {
            HostEntry::Bundle { name, .. } => name,
            HostEntry::Explicit { name, .. } => name,
        }
    }
}

/// Host manifest configuration (hosts.toml)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HostManifest {
    /// Global configuration
    #[serde(default)]
    pub global: GlobalConfig,

    /// List of hosts to load
    #[serde(default)]
    pub host: Vec<HostEntry>,
}

impl HostManifest {
    /// Load a host manifest from a file
    pub fn load_from_file(manifest_path: impl AsRef<Path>) -> Result<Self> {
        let manifest_path = manifest_path.as_ref();

        if !manifest_path.exists() {
            bail!("Host manifest not found: {}", manifest_path.display());
        }

        let manifest_content = std::fs::read_to_string(manifest_path)
            .with_context(|| format!("Failed to read manifest from {}", manifest_path.display()))?;

        let manifest: HostManifest = toml::from_str(&manifest_content)
            .with_context(|| format!("Failed to parse manifest from {}", manifest_path.display()))?;

        Ok(manifest)
    }

    /// Resolve a bundle name to a bundle path using search paths
    fn find_bundle(&self, bundle_name: &str, manifest_dir: &Path) -> Result<PathBuf> {
        // Try relative to manifest directory first
        let relative_path = manifest_dir.join(bundle_name);
        if relative_path.is_dir() && relative_path.join("host.toml").exists() {
            return Ok(relative_path);
        }

        // Try each search path
        for search_path in &self.global.search_paths {
            let search_path = if search_path.is_relative() {
                manifest_dir.join(search_path)
            } else {
                search_path.clone()
            };

            let bundle_path = search_path.join(bundle_name);
            if bundle_path.is_dir() && bundle_path.join("host.toml").exists() {
                return Ok(bundle_path);
            }
        }

        bail!(
            "Could not find bundle '{}' in search paths or relative to manifest",
            bundle_name
        );
    }

    /// Resolve all host entries to bundles
    pub fn resolve_bundles(&self, manifest_path: &Path) -> Result<Vec<HostBundle>> {
        let manifest_dir = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."));

        let mut bundles = Vec::new();

        for entry in &self.host {
            match entry {
                HostEntry::Bundle { bundle, .. } => {
                    let bundle_path = self.find_bundle(bundle, manifest_dir)?;
                    let host_bundle = HostBundle::load_from_dir(&bundle_path)?;
                    bundles.push(host_bundle);
                }
                HostEntry::Explicit { name, wit, lib } => {
                    // Create a synthetic bundle for explicit entries
                    let wit_path = if wit.is_relative() {
                        manifest_dir.join(wit)
                    } else {
                        wit.clone()
                    };

                    let lib_path = if lib.is_relative() {
                        manifest_dir.join(lib)
                    } else {
                        lib.clone()
                    };

                    if !wit_path.exists() {
                        bail!("WIT path not found for host '{}': {}", name, wit_path.display());
                    }

                    if !lib_path.exists() {
                        bail!("Library path not found for host '{}': {}", name, lib_path.display());
                    }

                    let config = HostBundleConfig {
                        host: HostConfig {
                            name: name.clone(),
                            lib: lib_path.clone(),
                            wit: wit_path.clone(),
                        },
                    };

                    // For explicit entries, the "bundle path" is the manifest directory
                    // and the paths in config are absolute
                    bundles.push(HostBundle {
                        config,
                        bundle_path: PathBuf::from("."),
                    });
                }
            }
        }

        Ok(bundles)
    }
}

/// Collection of loaded host bundles
#[derive(Debug, Default)]
pub struct HostBundles {
    bundles: Vec<HostBundle>,
}

impl HostBundles {
    /// Create a new empty collection
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a bundle from a directory path
    pub fn add_bundle_dir(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let bundle = HostBundle::load_from_dir(path)?;
        self.bundles.push(bundle);
        Ok(())
    }

    /// Add bundles from a manifest file
    pub fn add_manifest(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let manifest_path = path.as_ref();
        let manifest = HostManifest::load_from_file(manifest_path)?;
        let bundles = manifest.resolve_bundles(manifest_path)?;
        self.bundles.extend(bundles);
        Ok(())
    }

    /// Get all loaded bundles
    pub fn bundles(&self) -> &[HostBundle] {
        &self.bundles
    }

    /// Get a bundle by name
    pub fn get_bundle(&self, name: &str) -> Option<&HostBundle> {
        self.bundles.iter().find(|b| b.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_host_bundle_config_parsing() {
        let toml = r#"
            [host]
            name = "duckdb"
            lib = "lib/libduckdb_host.dylib"
            wit = "wit/duckdb-extension"
        "#;

        let config: HostBundleConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.host.name, "duckdb");
        assert_eq!(config.host.lib, PathBuf::from("lib/libduckdb_host.dylib"));
        assert_eq!(config.host.wit, PathBuf::from("wit/duckdb-extension"));
    }

    #[test]
    fn test_host_manifest_parsing() {
        let toml = r#"
            [global]
            search_paths = ["./hosts", "/usr/local/share/wasmtime/hosts"]

            [[host]]
            name = "duckdb"
            bundle = "duckdb_host"

            [[host]]
            name = "pkcs11"
            wit = "/opt/pkcs11-host/pkcs11.wit"
            lib = "/opt/pkcs11-host/libpkcs11_host.dylib"
        "#;

        let manifest: HostManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.global.search_paths.len(), 2);
        assert_eq!(manifest.host.len(), 2);

        match &manifest.host[0] {
            HostEntry::Bundle { name, bundle } => {
                assert_eq!(name, "duckdb");
                assert_eq!(bundle, "duckdb_host");
            }
            _ => panic!("Expected bundle entry"),
        }

        match &manifest.host[1] {
            HostEntry::Explicit { name, .. } => {
                assert_eq!(name, "pkcs11");
            }
            _ => panic!("Expected explicit entry"),
        }
    }
}
