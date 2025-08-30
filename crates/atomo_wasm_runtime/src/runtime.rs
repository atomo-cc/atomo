use anyhow::Result;
use wasmtime::{Engine, Store, Module, Instance, Linker};
use std::path::Path;

pub struct WasmRuntime {
    engine: Engine,
}

impl WasmRuntime {
    pub fn new() -> Result<Self> {
        let engine = Engine::default();
        Ok(Self { engine })
    }
    
    pub async fn load_plugin<P: AsRef<Path>>(&self, wasm_path: P) -> Result<WasmPlugin> {
        let mut store = Store::new(&self.engine, ());
        let module = Module::from_file(&self.engine, wasm_path)?;
        
        let linker = Linker::new(&self.engine);
        let instance = linker.instantiate(&mut store, &module)?;
        
        Ok(WasmPlugin {
            store,
            instance,
        })
    }
}

pub struct WasmPlugin {
    store: Store<()>,
    instance: Instance,
}

impl WasmPlugin {
    pub fn call_function(&mut self, name: &str, args: &[wasmtime::Val]) -> Result<Vec<wasmtime::Val>> {
        let func = self.instance
            .get_func(&mut self.store, name)
            .ok_or_else(|| anyhow::anyhow!("Function '{}' not found", name))?;
        
        let mut results = vec![wasmtime::Val::I32(0); func.ty(&self.store).results().len()];
        func.call(&mut self.store, args, &mut results)?;
        
        Ok(results)
    }
}
