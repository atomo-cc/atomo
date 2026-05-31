//! Tier-1 scripting runtime: run JavaScript plugins compiled with Javy/QuickJS.
//!
//! A Javy module embeds the QuickJS engine and communicates over WASI stdin/stdout:
//! we pipe the input record (JSON) to stdin, run the module's `_start`, and read the
//! transformed record (JSON) from stdout. Execution is fuel-metered like compiled plugins.
//!
//! Validated by the Phase 1 spike (see docs/guide/advanced/scripting-plugins-proposal.md):
//! a Javy plugin runs under wasmtime 16 + WASI with bounded fuel.

use anyhow::{Context, Result};
use std::path::Path;
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::WasiCtxBuilder;

/// Runtime for JavaScript (Javy-compiled) plugins.
pub struct JsRuntime {
    engine: Engine,
    fuel_limit: u64,
}

struct JsState {
    wasi: wasi_common::WasiCtx,
}

impl JsRuntime {
    pub fn new() -> Result<Self> {
        Self::with_fuel_limit(100_000_000)
    }

    pub fn with_fuel_limit(fuel_limit: u64) -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;
        Ok(Self { engine, fuel_limit })
    }

    /// Compile a Javy-built `.wasm` to a reusable `Module`. Do this once at load time;
    /// `Module` is cheap to clone and re-run, avoiding per-call recompilation.
    pub fn compile<P: AsRef<Path>>(&self, wasm_path: P) -> Result<Module> {
        Module::from_file(&self.engine, wasm_path).context("failed to load JS plugin wasm")
    }

    /// Run a pre-compiled JS plugin module with `input_json` on stdin; returns stdout.
    /// A fresh fuel-metered store + WASI context is created per call (isolated state).
    pub fn run_module(&self, module: &Module, input_json: &str) -> Result<String> {
        let stdin = wasi_common::pipe::ReadPipe::from(input_json.to_string());
        let stdout = wasi_common::pipe::WritePipe::new_in_memory();

        let wasi = WasiCtxBuilder::new()
            .stdin(Box::new(stdin))
            .stdout(Box::new(stdout.clone()))
            .inherit_stderr()
            .build();

        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s: &mut JsState| &mut s.wasi)?;

        let mut store = Store::new(&self.engine, JsState { wasi });
        store.set_fuel(self.fuel_limit)?;

        let instance = linker.instantiate(&mut store, module)?;
        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .context("JS plugin missing `_start` export (not a Javy module?)")?;
        start
            .call(&mut store, ())
            .context("JS plugin execution failed/trapped")?;

        // Drop the store to release the cloned stdout handle, then read the buffer.
        drop(store);
        let bytes = stdout
            .try_into_inner()
            .map_err(|_| anyhow::anyhow!("stdout still referenced"))?
            .into_inner();
        String::from_utf8(bytes).context("JS plugin stdout was not valid UTF-8")
    }

    /// Convenience: compile + run in one call (recompiles each time — prefer
    /// `compile` once + `run_module` per call on hot paths).
    pub fn run_js_plugin<P: AsRef<Path>>(&self, wasm_path: P, input_json: &str) -> Result<String> {
        let module = self.compile(wasm_path)?;
        self.run_module(&module, input_json)
    }
}
