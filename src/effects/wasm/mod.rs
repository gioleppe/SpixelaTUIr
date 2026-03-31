//! WASM-based effects plugin system.
//!
//! This module provides the ability to load user-supplied `.wasm` files as
//! custom image effects. Plugins are discovered from `~/.config/spix/plugins/`
//! at startup and appear alongside built-in effects in the add-effect menu.
//!
//! ## Plugin API Contract
//!
//! A valid WASM plugin must export these functions:
//!
//! | Export | Signature | Description |
//! |--------|-----------|-------------|
//! | `name` | `() -> i32` | Pointer to null-terminated UTF-8 effect name |
//! | `num_params` | `() -> i32` | Number of tunable parameters |
//! | `param_name` | `(i32) -> i32` | Pointer to null-terminated param name |
//! | `param_default` | `(i32) -> f32` | Default value for param at index |
//! | `param_min` | `(i32) -> f32` | Minimum value for param at index |
//! | `param_max` | `(i32) -> f32` | Maximum value for param at index |
//! | `set_param` | `(i32, f32) -> ()` | Set param value before processing |
//! | `process` | `(i32, i32, i32, i32) -> i32` | Process RGBA data (w, h, ptr, len), returns 0 on success |
//! | `alloc` | `(i32) -> i32` | Allocate bytes in WASM linear memory |
//! | `dealloc` | `(i32, i32) -> ()` | Free previously allocated bytes |
//! | `memory` | (memory export) | WASM linear memory |

pub mod registry;
pub mod runtime;

use std::fmt;

use image::{DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};

use super::ParamDescriptor;
use registry::get_registry;

/// A WASM-based image effect loaded from an external `.wasm` plugin.
///
/// Serialized pipelines store the plugin name (not file path) for portability.
/// If the plugin is not found at runtime, the effect passes the image through
/// unchanged and shows a warning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmEffect {
    /// Plugin name (matches the WASM module's exported `name()` function).
    pub plugin: String,
    /// Current parameter values, in the order defined by the plugin.
    pub params: Vec<f32>,
}

impl WasmEffect {
    /// Create a new `WasmEffect` with default parameter values from the registry.
    pub fn with_defaults(plugin_name: &str) -> Self {
        let params = get_registry()
            .and_then(|r| r.plugin_meta(plugin_name))
            .map(|m| m.params.iter().map(|p| p.default).collect())
            .unwrap_or_default();

        Self {
            plugin: plugin_name.to_string(),
            params,
        }
    }

    /// Whether the backing WASM plugin is present in the current registry.
    pub fn is_available(&self) -> bool {
        get_registry().is_some_and(|r| r.has_plugin(&self.plugin))
    }

    /// Apply this WASM effect to an entire image buffer.
    ///
    /// If the plugin is not found in the registry, the image is returned unchanged.
    pub fn apply_image(&self, img: DynamicImage) -> DynamicImage {
        let registry = match get_registry() {
            Some(r) => r,
            None => {
                log::warn!(
                    "WASM registry not initialized — skipping plugin '{}'",
                    self.plugin
                );
                return img;
            }
        };

        if !registry.has_plugin(&self.plugin) {
            log::warn!(
                "WASM plugin '{}' not found in registry — passing through",
                self.plugin
            );
            return img;
        }

        let (width, height) = img.dimensions();
        let mut rgba = img.into_rgba8();
        let raw = rgba.as_mut();

        match registry.execute(&self.plugin, &self.params, raw, width, height) {
            Ok(()) => DynamicImage::ImageRgba8(rgba),
            Err(e) => {
                log::error!("WASM plugin '{}' failed: {e:#}", self.plugin);
                DynamicImage::ImageRgba8(rgba)
            }
        }
    }

    /// Return descriptors for all tunable parameters of this WASM plugin.
    ///
    /// If the plugin is not found, returns an empty vec.
    pub fn param_descriptors(&self) -> Vec<ParamDescriptor> {
        let meta = get_registry().and_then(|r| r.plugin_meta(&self.plugin));

        match meta {
            Some(m) => m
                .params
                .iter()
                .enumerate()
                .map(|(i, pm)| ParamDescriptor {
                    name: pm.name,
                    value: self.params.get(i).copied().unwrap_or(pm.default),
                    min: pm.min,
                    max: pm.max,
                })
                .collect(),
            None => Vec::new(),
        }
    }

    /// Rebuild this effect with new parameter values, clamped to valid ranges.
    pub fn apply_params(&self, values: &[f32]) -> WasmEffect {
        let meta = get_registry().and_then(|r| r.plugin_meta(&self.plugin));

        let params = match meta {
            Some(m) => values
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    if let Some(pm) = m.params.get(i) {
                        v.clamp(pm.min, pm.max)
                    } else {
                        v
                    }
                })
                .collect(),
            None => values.to_vec(),
        };

        WasmEffect {
            plugin: self.plugin.clone(),
            params,
        }
    }

    /// Returns the plugin name for UI display.
    pub fn variant_name(&self) -> &str {
        // If the plugin is in the registry, use the leaked 'static name.
        // Otherwise fall back to the stored plugin name.
        get_registry()
            .and_then(|r| r.plugin_meta(&self.plugin))
            .map(|m| m.name as &str)
            .unwrap_or(&self.plugin)
    }
}

impl fmt::Display for WasmEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.params.is_empty() {
            write!(f, "WASM: {}", self.plugin)
        } else {
            write!(f, "WASM: {} (", self.plugin)?;
            for (i, v) in self.params.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{v}")?;
            }
            write!(f, ")")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_effect_serialize_roundtrip() {
        let effect = WasmEffect {
            plugin: "TestGlitch".to_string(),
            params: vec![0.5, 128.0, 1.0],
        };
        let json = serde_json::to_string(&effect).unwrap();
        let deserialized: WasmEffect = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.plugin, "TestGlitch");
        assert_eq!(deserialized.params, vec![0.5, 128.0, 1.0]);
    }

    #[test]
    fn wasm_effect_missing_plugin_passthrough() {
        // No registry initialized → apply_image should return image unchanged.
        let effect = WasmEffect {
            plugin: "NonExistent".to_string(),
            params: vec![],
        };
        let img = DynamicImage::new_rgba8(4, 4);
        let result = effect.apply_image(img.clone());
        assert_eq!(result.dimensions(), img.dimensions());
    }

    #[test]
    fn wasm_effect_param_descriptors_missing_plugin() {
        let effect = WasmEffect {
            plugin: "NonExistent".to_string(),
            params: vec![1.0, 2.0],
        };
        // No registry → should return empty descriptors.
        let descriptors = effect.param_descriptors();
        assert!(descriptors.is_empty());
    }

    #[test]
    fn wasm_effect_display_no_params() {
        let effect = WasmEffect {
            plugin: "MyEffect".to_string(),
            params: vec![],
        };
        assert_eq!(format!("{effect}"), "WASM: MyEffect");
    }

    #[test]
    fn wasm_effect_display_with_params() {
        let effect = WasmEffect {
            plugin: "MyEffect".to_string(),
            params: vec![0.5, 1.0],
        };
        assert_eq!(format!("{effect}"), "WASM: MyEffect (0.5, 1)");
    }

    #[test]
    fn wasm_effect_is_not_available_without_registry() {
        let effect = WasmEffect {
            plugin: "NonExistent".to_string(),
            params: vec![],
        };
        assert!(!effect.is_available());
    }

    #[test]
    fn wasm_effect_apply_params_no_registry() {
        let effect = WasmEffect {
            plugin: "X".to_string(),
            params: vec![1.0],
        };
        let updated = effect.apply_params(&[99.0]);
        assert_eq!(updated.params, vec![99.0]);
    }
}
