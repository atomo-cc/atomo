use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use wasmtime::{Caller, Config, Engine, Instance, Linker, Module, Store};

use crate::plugin::{Permission, PluginManifest};

pub struct PluginState {
    pub permissions: HashSet<Permission>,
    pub logs: Vec<String>,
}

pub struct WasmRuntime {
    engine: Engine,
    fuel_limit: u64,
}

impl WasmRuntime {
    pub fn new() -> Result<Self> {
        Self::with_fuel_limit(1_000_000)
    }

    pub fn with_fuel_limit(fuel_limit: u64) -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;
        Ok(Self { engine, fuel_limit })
    }

    pub async fn load_plugin<P: AsRef<Path>>(
        &self,
        wasm_path: P,
        manifest: &PluginManifest,
    ) -> Result<WasmPlugin> {
        let state = PluginState {
            permissions: manifest.permissions.iter().cloned().collect(),
            logs: Vec::new(),
        };
        let mut store = Store::new(&self.engine, state);
        store.set_fuel(self.fuel_limit)?;

        let module = Module::from_file(&self.engine, wasm_path)?;
        let mut linker = Linker::new(&self.engine);

        linker.func_wrap(
            "env",
            "host_log",
            |mut caller: Caller<'_, PluginState>, ptr: i32, len: i32| {
                let mem = caller.get_export("memory").and_then(|e| e.into_memory());
                if let Some(mem) = mem {
                    let data = mem.data(&caller);
                    if let Some(slice) = data.get(ptr as usize..(ptr + len) as usize) {
                        let msg = String::from_utf8_lossy(slice).to_string();
                        caller.data_mut().logs.push(msg);
                    }
                }
            },
        )?;

        linker.func_wrap(
            "env",
            "host_read_event",
            |caller: Caller<'_, PluginState>, _ptr: i32, _len: i32| -> Result<(), anyhow::Error> {
                if !caller.data().permissions.contains(&Permission::ReadEvents) {
                    anyhow::bail!("Permission denied: ReadEvents required");
                }
                Ok(())
            },
        )?;

        linker.func_wrap(
            "env",
            "host_emit_event",
            |caller: Caller<'_, PluginState>, _ptr: i32, _len: i32| -> Result<(), anyhow::Error> {
                if !caller.data().permissions.contains(&Permission::WriteEvents) {
                    anyhow::bail!("Permission denied: WriteEvents required");
                }
                Ok(())
            },
        )?;

        let instance = linker.instantiate(&mut store, &module)?;
        Ok(WasmPlugin { store, instance, fuel_limit: self.fuel_limit })
    }
}

pub struct WasmPlugin {
    store: Store<PluginState>,
    instance: Instance,
    fuel_limit: u64,
}

impl WasmPlugin {
    pub fn call_function(&mut self, name: &str, args: &[wasmtime::Val]) -> Result<Vec<wasmtime::Val>> {
        let func = self
            .instance
            .get_func(&mut self.store, name)
            .ok_or_else(|| anyhow::anyhow!("Function '{}' not found", name))?;
        let mut results = vec![wasmtime::Val::I32(0); func.ty(&self.store).results().len()];
        func.call(&mut self.store, args, &mut results)?;
        Ok(results)
    }

    pub fn check_permission(&self, perm: &Permission) -> Result<()> {
        if self.store.data().permissions.contains(perm) {
            Ok(())
        } else {
            anyhow::bail!("Permission denied: {:?}", perm)
        }
    }

    pub fn fuel_consumed(&self) -> u64 {
        self.fuel_limit - self.store.get_fuel().unwrap_or(0)
    }

    pub fn logs(&self) -> &[String] {
        &self.store.data().logs
    }
}
