//! Phase D4: CLI smoke test. Invokes the built `atomo` binary's `init` (self-contained file
//! scaffolding — no network/toolchain) and asserts the project structure is created.
//! `migrate`/`codegen` smoke are heavier (DB / full parser) and deferred.

use std::process::Command;

#[test]
fn init_scaffolds_a_project() {
    let bin = env!("CARGO_BIN_EXE_atomo-cli");
    let tmp = std::env::temp_dir().join(format!("atomo_cli_smoke_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let out = Command::new(bin)
        .args(["init", "my-app", "--template", "crm"])
        .current_dir(&tmp)
        .output()
        .expect("run atomo init");
    assert!(out.status.success(), "init failed: {}", String::from_utf8_lossy(&out.stderr));

    let proj = tmp.join("my-app");
    assert!(proj.join("atomo/schema.ts").exists(), "schema.ts not scaffolded");
    assert!(proj.join("package.json").exists(), "package.json not scaffolded");
    // The CRM template's schema should mention a CRM model.
    let schema = std::fs::read_to_string(proj.join("atomo/schema.ts")).unwrap();
    assert!(schema.contains("Contact") || schema.contains("Deal") || schema.contains("Company"),
        "crm template schema should contain a CRM model");

    std::fs::remove_dir_all(&tmp).ok();
}
