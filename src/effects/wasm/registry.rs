//! Plugin discovery and global registry.
//!
//! WASM plugins are discovered by scanning `~/.config/spix/plugins/` for
//! `.wasm` files at application startup. Each valid plugin is compiled once
//! and stored in a global `OnceLock<WasmPluginRegistry>` that is read-only
//! after initialization — no locking needed for concurrent reads.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use wasmer::{Engine, Module};

use super::runtime::{self, WasmPluginMeta};

/// Global plugin registry, initialized once at app startup.
static WASM_REGISTRY: OnceLock<WasmPluginRegistry> = OnceLock::new();

/// Initialize the global WASM plugin registry by scanning the plugins directory.
///
/// This must be called once at app startup (before the worker thread starts).
/// Subsequent calls are no-ops.
pub fn init_registry() {
    let plugins_dir = dirs::config_dir()
        .map(|d| d.join("spix").join("plugins"))
        .unwrap_or_else(|| PathBuf::from("plugins"));

    if !plugins_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&plugins_dir) {
            log::warn!(
                "Failed to create plugin directory {}: {e}",
                plugins_dir.display()
            );
        } else {
            log::info!("Created plugin directory: {}", plugins_dir.display());
        }
    }

    let registry = WasmPluginRegistry::discover(&plugins_dir);
    log::info!(
        "WASM registry: {} plugin(s) loaded",
        registry.plugin_count()
    );

    let _ = WASM_REGISTRY.set(registry);
}

/// Get a reference to the global WASM plugin registry.
///
/// Returns `None` if [`init_registry`] has not been called yet.
pub fn get_registry() -> Option<&'static WasmPluginRegistry> {
    WASM_REGISTRY.get()
}

/// Registry of compiled WASM effect plugins.
///
/// After construction via [`discover`], this is immutable.
/// `wasmer::Engine` and `wasmer::Module` are both `Send + Sync`,
/// so the registry can safely live in a `OnceLock` static.
pub struct WasmPluginRegistry {
    engine: Engine,
    /// Map from plugin name → (compiled module, metadata).
    plugins: HashMap<String, (Module, WasmPluginMeta)>,
    /// Ordered list of plugin names for deterministic UI ordering.
    plugin_order: Vec<String>,
}

// Compile-time verification that wasmer's Engine and Module are Send + Sync.
// This is required for safe storage in the global OnceLock registry.
// If wasmer ever removes Send/Sync from these types, this will fail to compile.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Engine>();
    assert_send_sync::<Module>();
};

impl std::fmt::Debug for WasmPluginRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmPluginRegistry")
            .field("plugin_count", &self.plugins.len())
            .field("plugin_order", &self.plugin_order)
            .finish()
    }
}

impl WasmPluginRegistry {
    /// Scan `plugins_dir` for `.wasm` files, compile each, and build the registry.
    ///
    /// Invalid or duplicate plugins are logged and skipped.
    pub fn discover(plugins_dir: &Path) -> Self {
        let engine = Engine::default();
        let mut plugins = HashMap::new();
        let mut plugin_order = Vec::new();

        let entries = match std::fs::read_dir(plugins_dir) {
            Ok(entries) => entries,
            Err(e) => {
                log::warn!(
                    "Cannot read plugin directory {}: {e}",
                    plugins_dir.display()
                );
                return Self {
                    engine,
                    plugins,
                    plugin_order,
                };
            }
        };

        // Collect and sort entries alphabetically for deterministic ordering.
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "wasm"))
            .collect();
        paths.sort();

        for path in paths {
            match runtime::compile_plugin(&engine, &path) {
                Ok((module, meta)) => {
                    let name = meta.name.to_string();
                    if plugins.contains_key(&name) {
                        log::warn!(
                            "Duplicate WASM plugin name '{}' from {} (skipping — first wins)",
                            name,
                            path.display()
                        );
                        continue;
                    }
                    log::info!("Registered WASM plugin '{}' from {}", name, path.display());
                    plugin_order.push(name.clone());
                    plugins.insert(name, (module, meta));
                }
                Err(e) => {
                    log::warn!("Failed to load WASM plugin {}: {e:#}", path.display());
                }
            }
        }

        Self {
            engine,
            plugins,
            plugin_order,
        }
    }

    /// Number of successfully loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Ordered list of plugin names for the UI menu.
    pub fn list_plugins(&self) -> &[String] {
        &self.plugin_order
    }

    /// Look up a plugin's metadata by name.
    pub fn plugin_meta(&self, name: &str) -> Option<&WasmPluginMeta> {
        self.plugins.get(name).map(|(_, meta)| meta)
    }

    /// Check whether a plugin with the given name is registered.
    pub fn has_plugin(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Execute a plugin by name on raw RGBA pixel data.
    pub fn execute(
        &self,
        name: &str,
        params: &[f32],
        rgba: &mut [u8],
        width: u32,
        height: u32,
    ) -> anyhow::Result<()> {
        let (module, _meta) = self
            .plugins
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("WASM plugin '{}' not found in registry", name))?;

        runtime::execute_plugin(&self.engine, module, params, rgba, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn discover_empty_dir_returns_empty_registry() {
        let dir = TempDir::new().unwrap();
        let registry = WasmPluginRegistry::discover(dir.path());
        assert_eq!(registry.plugin_count(), 0);
        assert!(registry.list_plugins().is_empty());
    }

    #[test]
    fn discover_invalid_wasm_skipped() {
        let dir = TempDir::new().unwrap();
        // Write garbage bytes to a .wasm file.
        std::fs::write(dir.path().join("bad_plugin.wasm"), b"not valid wasm").unwrap();
        let registry = WasmPluginRegistry::discover(dir.path());
        // Invalid plugin should be skipped, not panic.
        assert_eq!(registry.plugin_count(), 0);
    }

    #[test]
    fn discover_nonexistent_dir_returns_empty_registry() {
        let registry = WasmPluginRegistry::discover(Path::new("/nonexistent/dir/wasm/plugins"));
        assert_eq!(registry.plugin_count(), 0);
    }

    #[test]
    fn has_plugin_returns_false_for_unknown() {
        let dir = TempDir::new().unwrap();
        let registry = WasmPluginRegistry::discover(dir.path());
        assert!(!registry.has_plugin("SomePlugin"));
    }
}
