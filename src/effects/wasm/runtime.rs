//! Low-level wasmer interactions — compile WASM modules, instantiate them,
//! and call exported functions for the plugin API contract.

use std::path::Path;

use anyhow::{Context, Result, bail};
use wasmer::{Engine, Imports, Instance, Memory, Module, Store, TypedFunction};

/// Cached metadata for one tunable parameter of a WASM plugin.
#[derive(Debug, Clone)]
pub struct WasmParamMeta {
    pub name: &'static str,
    pub default: f32,
    pub min: f32,
    pub max: f32,
}

/// All metadata for a single WASM plugin, extracted at compile/load time.
#[derive(Debug, Clone)]
pub struct WasmPluginMeta {
    /// Human-readable name exported by the plugin's `name()` function.
    /// Leaked to `'static` so it can be used as `variant_name()`.
    pub name: &'static str,
    /// Original file path for logging/diagnostics.
    pub file_path: std::path::PathBuf,
    /// Parameter metadata (name, default, min, max) for each tunable param.
    pub params: Vec<WasmParamMeta>,
}

/// Compile a `.wasm` file, extract its metadata by calling the plugin's
/// query exports (`name`, `num_params`, `param_name`, etc.) in a temporary
/// `Store`/`Instance`, and return the compiled `Module` + cached metadata.
///
/// The temporary `Store`/`Instance` is dropped after metadata extraction.
pub fn compile_plugin(engine: &Engine, path: &Path) -> Result<(Module, WasmPluginMeta)> {
    let wasm_bytes =
        std::fs::read(path).with_context(|| format!("reading WASM plugin {}", path.display()))?;

    let module = Module::new(engine, &wasm_bytes)
        .with_context(|| format!("compiling WASM plugin {}", path.display()))?;

    // Temporary store + instance to query metadata exports.
    let mut store = Store::new(engine.clone());
    let imports = Imports::new();
    let instance = Instance::new(&mut store, &module, &imports)
        .with_context(|| format!("instantiating WASM plugin {}", path.display()))?;

    // Validate required exports exist.
    validate_exports(&instance, &mut store, path)?;

    // Read the effect name.
    let name_fn: TypedFunction<(), i32> = instance
        .exports
        .get_typed_function(&store, "name")
        .context("getting `name` export")?;
    let name_ptr = name_fn.call(&mut store).context("calling `name()`")?;
    let name_str = read_wasm_string(&instance, &store, name_ptr)
        .context("reading name string from WASM memory")?;
    // Leak to 'static for use in variant_name() / ParamDescriptor.
    let name: &'static str = Box::leak(name_str.into_boxed_str());

    // Read parameter count.
    let num_params_fn: TypedFunction<(), i32> = instance
        .exports
        .get_typed_function(&store, "num_params")
        .context("getting `num_params` export")?;
    let num_params = num_params_fn
        .call(&mut store)
        .context("calling `num_params()`")?;

    let mut params = Vec::with_capacity(num_params.max(0) as usize);

    let param_name_fn: TypedFunction<i32, i32> = instance
        .exports
        .get_typed_function(&store, "param_name")
        .context("getting `param_name` export")?;
    let param_default_fn: TypedFunction<i32, f32> = instance
        .exports
        .get_typed_function(&store, "param_default")
        .context("getting `param_default` export")?;
    let param_min_fn: TypedFunction<i32, f32> = instance
        .exports
        .get_typed_function(&store, "param_min")
        .context("getting `param_min` export")?;
    let param_max_fn: TypedFunction<i32, f32> = instance
        .exports
        .get_typed_function(&store, "param_max")
        .context("getting `param_max` export")?;

    for i in 0..num_params {
        let pname_ptr = param_name_fn
            .call(&mut store, i)
            .with_context(|| format!("calling `param_name({i})`"))?;
        let pname = read_wasm_string(&instance, &store, pname_ptr)
            .with_context(|| format!("reading param_name({i}) from WASM memory"))?;
        let pname: &'static str = Box::leak(pname.into_boxed_str());

        let default = param_default_fn
            .call(&mut store, i)
            .with_context(|| format!("calling `param_default({i})`"))?;
        let min = param_min_fn
            .call(&mut store, i)
            .with_context(|| format!("calling `param_min({i})`"))?;
        let max = param_max_fn
            .call(&mut store, i)
            .with_context(|| format!("calling `param_max({i})`"))?;

        params.push(WasmParamMeta {
            name: pname,
            default,
            min,
            max,
        });
    }

    let meta = WasmPluginMeta {
        name,
        file_path: path.to_path_buf(),
        params,
    };

    log::info!(
        "Compiled WASM plugin '{}' from {} ({} params)",
        meta.name,
        path.display(),
        meta.params.len()
    );

    Ok((module, meta))
}

/// Execute a compiled WASM plugin on raw RGBA pixel data.
///
/// Creates a fresh `Store` + `Instance` for each invocation (wasmer `Store`
/// is lightweight). Sets parameters via `set_param`, allocates WASM memory
/// via `alloc`, copies pixel data in, calls `process`, and copies back.
pub fn execute_plugin(
    engine: &Engine,
    module: &Module,
    params: &[f32],
    rgba: &mut [u8],
    width: u32,
    height: u32,
) -> Result<()> {
    let mut store = Store::new(engine.clone());
    let imports = Imports::new();
    let instance = Instance::new(&mut store, module, &imports)
        .context("instantiating WASM plugin for execution")?;

    // Set parameters.
    let set_param_fn: TypedFunction<(i32, f32), ()> = instance
        .exports
        .get_typed_function(&store, "set_param")
        .context("getting `set_param` export")?;

    for (i, &val) in params.iter().enumerate() {
        set_param_fn
            .call(&mut store, i as i32, val)
            .with_context(|| format!("calling `set_param({i}, {val})`"))?;
    }

    // Allocate WASM memory for pixel data.
    let len = rgba.len() as i32;
    let alloc_fn: TypedFunction<i32, i32> = instance
        .exports
        .get_typed_function(&store, "alloc")
        .context("getting `alloc` export")?;
    let ptr = alloc_fn
        .call(&mut store, len)
        .context("calling `alloc()`")?;

    if ptr < 0 {
        bail!("WASM alloc returned negative pointer: {ptr}");
    }

    // Copy pixel data into WASM memory.
    let memory: &Memory = instance
        .exports
        .get_memory("memory")
        .context("getting WASM memory export")?;

    let mem_view = memory.view(&store);
    mem_view
        .write(ptr as u64, rgba)
        .context("writing pixel data to WASM memory")?;

    // Call process(width, height, ptr, len).
    let process_fn: TypedFunction<(i32, i32, i32, i32), i32> = instance
        .exports
        .get_typed_function(&store, "process")
        .context("getting `process` export")?;

    let result = process_fn
        .call(&mut store, width as i32, height as i32, ptr, len)
        .context("calling `process()`")?;

    if result != 0 {
        bail!("WASM process() returned error code: {result}");
    }

    // Read modified pixels back from WASM memory.
    let mem_view = memory.view(&store);
    mem_view
        .read(ptr as u64, rgba)
        .context("reading pixel data from WASM memory")?;

    // Deallocate WASM memory.
    let dealloc_fn: TypedFunction<(i32, i32), ()> = instance
        .exports
        .get_typed_function(&store, "dealloc")
        .context("getting `dealloc` export")?;
    dealloc_fn
        .call(&mut store, ptr, len)
        .context("calling `dealloc()`")?;

    Ok(())
}

/// Read a null-terminated UTF-8 string from WASM linear memory at the given pointer.
fn read_wasm_string(instance: &Instance, store: &Store, ptr: i32) -> Result<String> {
    let memory: &Memory = instance
        .exports
        .get_memory("memory")
        .context("getting WASM memory for string read")?;

    let mem_view = memory.view(store);
    let mem_size = mem_view.data_size() as i32;

    if ptr < 0 || ptr >= mem_size {
        bail!("WASM string pointer {ptr} out of bounds (memory size: {mem_size})");
    }

    // Read bytes until null terminator or end of memory.
    let max_len = (mem_size - ptr) as usize;
    let max_len = max_len.min(4096); // Safety cap: 4 KiB max string length.
    let mut bytes = vec![0u8; max_len];
    mem_view
        .read(ptr as u64, &mut bytes)
        .context("reading string bytes from WASM memory")?;

    let nul_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let s =
        String::from_utf8(bytes[..nul_pos].to_vec()).context("WASM string is not valid UTF-8")?;
    Ok(s)
}

/// Validate that a WASM instance exports all required functions.
fn validate_exports(instance: &Instance, _store: &mut Store, path: &Path) -> Result<()> {
    let required_fns = [
        "name",
        "num_params",
        "param_name",
        "param_default",
        "param_min",
        "param_max",
        "set_param",
        "process",
        "alloc",
        "dealloc",
    ];

    for name in &required_fns {
        instance.exports.get_function(name).with_context(|| {
            format!(
                "WASM plugin {} missing required export `{name}`",
                path.display()
            )
        })?;
    }

    // Validate memory export.
    instance.exports.get_memory("memory").with_context(|| {
        format!(
            "WASM plugin {} missing required `memory` export",
            path.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_wasm_string_handles_invalid_ptr() {
        // This test verifies that our string reading code handles edge cases
        // gracefully — we can't easily instantiate a real WASM module in a
        // unit test without a fixture, so we test the error path.
        let engine = Engine::default();
        let module_bytes = wat_identity_module();
        let module = Module::new(&engine, &module_bytes).unwrap();
        let mut store = Store::new(engine);
        let imports = Imports::new();
        let instance = Instance::new(&mut store, &module, &imports).unwrap();

        // Pointer far beyond memory bounds should error.
        let result = read_wasm_string(&instance, &store, i32::MAX);
        assert!(result.is_err());
    }

    /// Minimal WAT module with only memory export for testing.
    fn wat_identity_module() -> Vec<u8> {
        wat::parse_str(
            r#"(module
                (memory (export "memory") 1)
            )"#,
        )
        .expect("valid WAT")
    }
}
