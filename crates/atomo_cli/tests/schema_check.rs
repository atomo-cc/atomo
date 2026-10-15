use std::process::Command;

#[test]
fn schema_check_accepts_constraint_call_names_in_sql_strings() {
    let output = Command::new(env!("CARGO_BIN_EXE_atomo-cli"))
        .args([
            "schema",
            "check",
            "--schema",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/constraint-sql-strings.ts"
            ),
        ])
        .output()
        .expect("run the real schema check CLI");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 model(s), 0 action(s)"), "{stdout}");
    assert!(stdout.contains("no schema warnings"), "{stdout}");
}
