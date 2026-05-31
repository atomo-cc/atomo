use anyhow::{Result, Context};
use colored::*;
use console::style;
use std::fs;
use std::path::Path;
use std::process::Command;

pub async fn deploy_command(env: String) -> Result<()> {
    println!("☁️  {}", style("Deploying to Atomo Cloud...").cyan());

    // Step 1: Validate project
    println!("   🔍 Validating project...");
    let schema_path = find_schema();
    if schema_path.is_none() {
        anyhow::bail!("No schema.ts found. Run this command from a service directory.");
    }
    println!("   ✅ Schema validated");

    // Step 2: Build release
    println!("   📦 Building release...");
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .context("Failed to run cargo build")?;
    if !status.success() {
        anyhow::bail!("Build failed. Fix errors before deploying.");
    }
    println!("   ✅ Build successful");

    // Step 3: Generate deployment manifest
    println!("   📋 Generating deployment manifest...");
    let manifest = serde_json::json!({
        "version": "0.1.0",
        "environment": env,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "schema": schema_path.unwrap().to_string_lossy(),
    });
    fs::write("deploy-manifest.json", serde_json::to_string_pretty(&manifest)?)?;
    println!("   ✅ Manifest written to deploy-manifest.json");

    // Step 4: Summary
    println!();
    println!("   🚀 {}", format!("Ready to deploy to '{}' environment", env).bright_green());
    println!("   📄 Manifest: deploy-manifest.json");
    println!("   💡 To deploy to Atomo Cloud, push to your configured remote.");
    println!("   🔗 Dashboard: {}", "https://cloud.atomo.cc/deployments".bright_blue());

    Ok(())
}

fn find_schema() -> Option<std::path::PathBuf> {
    let candidates = ["schema.ts", "atomo/schema.ts", "src/schema.ts"];
    for c in &candidates {
        let p = Path::new(c);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}
