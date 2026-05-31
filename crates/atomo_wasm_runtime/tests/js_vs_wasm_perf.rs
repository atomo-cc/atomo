//! Phase 3 §9: rough perf comparison — JS (Javy) vs compiled wasm.
//! Not a microbenchmark harness; it prints wall-clock numbers to compare orders of
//! magnitude. Run: `cargo test -p atomo_wasm_runtime --test js_vs_wasm_perf -- --ignored --nocapture`
//!
//! Measured (one machine, debug build):
//! - cold start = compile the module from bytes (the once-per-load cost)
//! - per call   = instantiate + run the hook (the per-invocation cost)
//!
//! The takeaway documented in the proposal: JS cold-start is dominated by compiling the
//! ~1.2MB Javy/QuickJS module (far larger than a hand-written wasm); per-call both pay
//! fresh-instance + fuel setup, with JS additionally running the JS interpreter.

use std::time::Instant;

use atomo_wasm_runtime::JsRuntime;
use wasmtime::{Config, Engine, Module, Store};

const ITERS: u32 = 50;

fn js_fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tag_plugin.wasm")
}

// A minimal compiled plugin: a `run` export that does trivial work.
const TINY_WAT: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "run") (param i32 i32) (result i32) (i32.add (local.get 0) (local.get 1)))
)"#;

#[test]
#[ignore]
fn js_vs_wasm_perf() {
    // --- JS (Javy) ---
    let js = JsRuntime::new().unwrap();
    let bytes = std::fs::read(js_fixture()).unwrap();
    let t = Instant::now();
    let js_module = js.compile(js_fixture()).unwrap();
    let js_cold = t.elapsed();

    let t = Instant::now();
    for _ in 0..ITERS {
        let _ = js.run_module(&js_module, r#"{"tags":[]}"#).unwrap();
    }
    let js_per_call = t.elapsed() / ITERS;

    // --- Compiled wasm ---
    let mut cfg = Config::new();
    cfg.consume_fuel(true);
    let engine = Engine::new(&cfg).unwrap();
    let wasm_bytes = wat::parse_str(TINY_WAT).unwrap();
    let t = Instant::now();
    let wasm_module = Module::from_binary(&engine, &wasm_bytes).unwrap();
    let wasm_cold = t.elapsed();

    let t = Instant::now();
    for _ in 0..ITERS {
        let mut store = Store::new(&engine, ());
        store.set_fuel(1_000_000).unwrap();
        let inst = wasmtime::Instance::new(&mut store, &wasm_module, &[]).unwrap();
        let f = inst.get_typed_func::<(i32, i32), i32>(&mut store, "run").unwrap();
        let _ = f.call(&mut store, (1, 2)).unwrap();
    }
    let wasm_per_call = t.elapsed() / ITERS;

    println!("\n=== JS (Javy) vs compiled wasm (debug build, {ITERS} iters) ===");
    println!("module size:   JS {:>8} B   |  compiled {:>6} B", bytes.len(), wasm_bytes.len());
    println!("cold start:    JS {:>8?}   |  compiled {:>8?}", js_cold, wasm_cold);
    println!("per call:      JS {:>8?}   |  compiled {:>8?}", js_per_call, wasm_per_call);
    println!("ratio:         cold {:.0}x   per-call {:.0}x",
        js_cold.as_secs_f64() / wasm_cold.as_secs_f64().max(1e-9),
        js_per_call.as_secs_f64() / wasm_per_call.as_secs_f64().max(1e-9));
}
