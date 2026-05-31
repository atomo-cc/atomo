use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use wasmtime::{Caller, Config, Engine, Instance, Linker, Module, Store};

use crate::plugin::{Permission, PluginManifest};

pub struct PluginState {
    pub permissions: HashSet<Permission>,
    pub logs: Vec<String>,
    /// Events the plugin emitted (JSON strings), gated by WriteEvents.
    pub emitted_events: Vec<String>,
    /// Events made available to the plugin to read (JSON strings), gated by ReadEvents.
    pub readable_events: Vec<String>,
    /// Cursor for sequential host_read_event consumption.
    pub read_cursor: usize,
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
            emitted_events: Vec::new(),
            readable_events: Vec::new(),
            read_cursor: 0,
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
            |mut caller: Caller<'_, PluginState>, ptr: i32, cap: i32| -> Result<i32, anyhow::Error> {
                if !caller.data().permissions.contains(&Permission::ReadEvents) {
                    anyhow::bail!("Permission denied: ReadEvents required");
                }
                let cursor = caller.data().read_cursor;
                if cursor >= caller.data().readable_events.len() {
                    return Ok(0);
                }
                let event_bytes = caller.data().readable_events[cursor].clone().into_bytes();
                let write_len = (cap as usize).min(event_bytes.len());
                caller.data_mut().read_cursor += 1;
                let mem = caller.get_export("memory").and_then(|e| e.into_memory());
                if let Some(mem) = mem {
                    mem.write(&mut caller, ptr as usize, &event_bytes[..write_len])?;
                }
                Ok(write_len as i32)
            },
        )?;

        linker.func_wrap(
            "env",
            "host_emit_event",
            |mut caller: Caller<'_, PluginState>, ptr: i32, len: i32| -> Result<(), anyhow::Error> {
                if !caller.data().permissions.contains(&Permission::WriteEvents) {
                    anyhow::bail!("Permission denied: WriteEvents required");
                }
                let mem = caller.get_export("memory").and_then(|e| e.into_memory());
                if let Some(mem) = mem {
                    let data = mem.data(&caller);
                    if let Some(slice) = data.get(ptr as usize..(ptr + len) as usize) {
                        let s = String::from_utf8_lossy(slice).to_string();
                        caller.data_mut().emitted_events.push(s);
                    }
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

    pub fn emitted_events(&self) -> &[String] {
        &self.store.data().emitted_events
    }

    /// Seed events the plugin can read via host_read_event.
    pub fn set_readable_events(&mut self, events: Vec<String>) {
        self.store.data_mut().readable_events = events;
        self.store.data_mut().read_cursor = 0;
    }

    /// Call a hook function passing JSON input, returning optional JSON output.
    /// ABI: guest exports `alloc(i32)->i32` and `{hook}(ptr:i32,len:i32)->i64`.
    /// Return value packs ptr<<32 | len; 0 means 'no modification'.
    pub fn call_hook(&mut self, hook_name: &str, input_json: &str) -> Result<Option<String>> {
        let memory = self.instance.get_memory(&mut self.store, "memory")
            .ok_or_else(|| anyhow::anyhow!("plugin has no exported memory"))?;

        let input_bytes = input_json.as_bytes();
        let len = input_bytes.len() as i32;
        let ptr = match self.instance.get_typed_func::<i32, i32>(&mut self.store, "alloc") {
            Ok(alloc) => alloc.call(&mut self.store, len)?,
            Err(_) => return Err(anyhow::anyhow!("plugin missing required `alloc` export")),
        };

        memory.write(&mut self.store, ptr as usize, input_bytes)?;

        let hook = match self.instance.get_typed_func::<(i32, i32), i64>(&mut self.store, hook_name) {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        let packed = hook.call(&mut self.store, (ptr, len))?;
        if packed == 0 {
            return Ok(None);
        }

        let out_ptr = (packed >> 32) as u32 as usize;
        let out_len = (packed & 0xFFFF_FFFF) as u32 as usize;
        let mut buf = vec![0u8; out_len];
        memory.read(&self.store, out_ptr, &mut buf)?;
        let out = String::from_utf8(buf)
            .map_err(|e| anyhow::anyhow!("invalid utf8 from plugin: {}", e))?;
        Ok(Some(out))
    }
}
