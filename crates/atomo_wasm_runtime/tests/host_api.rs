use atomo_wasm_runtime::{Permission, PluginManifest, WasmRuntime};
use std::io::Write;
use wasmtime::Val;

const WAT: &str = r#"(module
  (import "env" "host_log" (func $host_log (param i32 i32)))
  (import "env" "host_emit_event" (func $host_emit (param i32 i32)))
  (import "env" "host_db_query" (func $host_db (param i32 i32)))
  (import "env" "host_http_request" (func $host_http (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 100) "{\"k\":1}")
  (data (i32.const 200) "hello-log")
  (data (i32.const 300) "{\"sql\":\"SELECT 1\"}")
  (data (i32.const 340) "{\"method\":\"GET\",\"url\":\"http://x\"}")
  (func (export "add") (param i32 i32) (result i32) (i32.add (local.get 0) (local.get 1)))
  (func (export "alloc") (param i32) (result i32) (i32.const 1024))
  (func (export "do_log") (call $host_log (i32.const 200) (i32.const 9)))
  (func (export "do_emit") (call $host_emit (i32.const 100) (i32.const 7)))
  (func (export "do_db") (call $host_db (i32.const 300) (i32.const 18)))
  (func (export "do_http") (call $host_http (i32.const 340) (i32.const 33)))
  (func (export "before_create") (param i32 i32) (result i64) (i64.const 0))
)"#;

fn manifest(perms: Vec<Permission>) -> PluginManifest {
    PluginManifest {
        name: "t".into(),
        version: "0".into(),
        description: "".into(),
        author: "".into(),
        entry_point: "t.wasm".into(),
        permissions: perms,
    }
}

fn wasm_path() -> std::path::PathBuf {
    let wasm = wat::parse_str(WAT).expect("WAT parse failed");
    let path = std::env::temp_dir().join(format!(
        "atomo_test_{}.wasm",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(&wasm).unwrap();
    path
}

#[tokio::test]
async fn call_function_add() {
    let rt = WasmRuntime::new().unwrap();
    let path = wasm_path();
    let mut plugin = rt.load_plugin(&path, &manifest(vec![])).await.unwrap();
    let res = plugin
        .call_function("add", &[Val::I32(2), Val::I32(3)])
        .unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].i32(), Some(5));
    assert!(plugin.fuel_consumed() > 0);
}

#[tokio::test]
async fn host_log_captured() {
    let rt = WasmRuntime::new().unwrap();
    let path = wasm_path();
    let mut plugin = rt.load_plugin(&path, &manifest(vec![])).await.unwrap();
    plugin.call_function("do_log", &[]).unwrap();
    assert!(plugin.logs().contains(&"hello-log".to_string()));
}

#[tokio::test]
async fn emit_denied_without_permission() {
    let rt = WasmRuntime::new().unwrap();
    let path = wasm_path();
    let mut plugin = rt.load_plugin(&path, &manifest(vec![])).await.unwrap();
    let res = plugin.call_function("do_emit", &[]);
    assert!(
        res.is_err(),
        "emit should fail without WriteEvents permission"
    );
}

#[tokio::test]
async fn emit_allowed_with_permission() {
    let rt = WasmRuntime::new().unwrap();
    let path = wasm_path();
    let mut plugin = rt
        .load_plugin(&path, &manifest(vec![Permission::WriteEvents]))
        .await
        .unwrap();
    plugin.call_function("do_emit", &[]).unwrap();
    assert_eq!(plugin.emitted_events(), &[r#"{"k":1}"#.to_string()]);
}

#[tokio::test]
async fn call_hook_returns_none() {
    let rt = WasmRuntime::new().unwrap();
    let path = wasm_path();
    let mut plugin = rt.load_plugin(&path, &manifest(vec![])).await.unwrap();
    let result = plugin.call_hook("before_create", "{}").unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn db_query_denied_without_permission() {
    let rt = WasmRuntime::new().unwrap();
    let path = wasm_path();
    let mut plugin = rt.load_plugin(&path, &manifest(vec![])).await.unwrap();
    assert!(
        plugin.call_function("do_db", &[]).is_err(),
        "db query should fail without ReadDatabase permission"
    );
}

#[tokio::test]
async fn db_query_recorded_with_permission() {
    let rt = WasmRuntime::new().unwrap();
    let path = wasm_path();
    let mut plugin = rt
        .load_plugin(&path, &manifest(vec![Permission::ReadDatabase]))
        .await
        .unwrap();
    plugin.call_function("do_db", &[]).unwrap();
    assert_eq!(plugin.db_requests(), &[r#"{"sql":"SELECT 1"}"#.to_string()]);
}

#[tokio::test]
async fn http_request_denied_without_permission() {
    let rt = WasmRuntime::new().unwrap();
    let path = wasm_path();
    let mut plugin = rt.load_plugin(&path, &manifest(vec![])).await.unwrap();
    assert!(
        plugin.call_function("do_http", &[]).is_err(),
        "http request should fail without HttpRequests permission"
    );
}

#[tokio::test]
async fn http_request_recorded_with_permission() {
    let rt = WasmRuntime::new().unwrap();
    let path = wasm_path();
    let mut plugin = rt
        .load_plugin(&path, &manifest(vec![Permission::HttpRequests]))
        .await
        .unwrap();
    plugin.call_function("do_http", &[]).unwrap();
    assert_eq!(
        plugin.http_requests(),
        &[r#"{"method":"GET","url":"http://x"}"#.to_string()]
    );
}
