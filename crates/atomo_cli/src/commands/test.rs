use anyhow::Result;
use colored::*;
use std::process::Command;
use std::path::Path;

pub async fn test_command(service_path: Option<String>, filter: Option<String>) -> Result<()> {
    println!("🧪 {}", "Running service tests...".cyan());

    let dir = service_path.unwrap_or_else(|| ".".to_string());
    let dir_path = Path::new(&dir);

    let test_dir = dir_path.join("tests");
    if !test_dir.exists() {
        println!("   ⚠️  No tests/ directory found in {}", dir);
        println!("   💡 Create tests in tests/ directory to get started");
        return Ok(());
    }

    let mut cmd = Command::new("cargo");
    cmd.arg("test");

    if let Some(f) = &filter {
        cmd.arg(f);
    }

    cmd.arg("--").arg("--nocapture");
    cmd.current_dir(dir_path);

    println!("   🔨 Running: cargo test {}", filter.as_deref().unwrap_or(""));

    let status = cmd.status()?;

    if status.success() {
        println!("   ✅ {}", "All tests passed!".bright_green());
    } else {
        println!("   ❌ {}", "Some tests failed".bright_red());
        std::process::exit(1);
    }

    Ok(())
}
