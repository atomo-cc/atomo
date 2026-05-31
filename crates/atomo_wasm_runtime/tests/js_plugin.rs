//! Tier-1 JS plugin integration test (Phase 2 M1).
//!
//! Runs a prebuilt Javy plugin (`tests/fixtures/tag_plugin.wasm`) through `JsRuntime`:
//! it reads a JSON record from stdin, adds a "js" tag, and writes JSON to stdout.
//! The fixture is committed so the test needs no Javy toolchain.

use atomo_wasm_runtime::JsRuntime;

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tag_plugin.wasm")
}

#[test]
fn js_plugin_transforms_record_via_stdin_stdout() {
    let rt = JsRuntime::new().unwrap();
    let out = rt
        .run_js_plugin(fixture(), r#"{"email":"a@b.com","tags":["x"]}"#)
        .expect("js plugin run failed");
    let v: serde_json::Value = serde_json::from_str(&out).expect("output is JSON");
    let tags = v["tags"].as_array().expect("tags array");
    assert!(tags.iter().any(|t| t == "x"), "original tag preserved");
    assert!(tags.iter().any(|t| t == "js"), "js plugin added its tag");
    assert_eq!(v["email"], "a@b.com");
}

#[test]
fn js_plugin_handles_empty_record() {
    let rt = JsRuntime::new().unwrap();
    let out = rt.run_js_plugin(fixture(), "{}").expect("run failed");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["tags"], serde_json::json!(["js"]));
}

#[test]
fn js_plugin_is_fuel_metered() {
    // A tiny fuel budget must abort the JS engine (proves metering is active).
    let rt = JsRuntime::with_fuel_limit(1_000).unwrap();
    let res = rt.run_js_plugin(fixture(), r#"{"tags":[]}"#);
    assert!(res.is_err(), "expected fuel exhaustion to trap the plugin");
}
