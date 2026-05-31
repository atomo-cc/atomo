use anyhow::Result;
use colored::*;
use console::style;
use std::fs;
use std::path::Path;

pub async fn init_command(name: String, template: Option<String>) -> Result<()> {
    println!("📦 {}", style("Initializing new Atomo project...").cyan());
    
    // Create project directory
    let project_path = Path::new(&name);
    if project_path.exists() {
        return Err(anyhow::anyhow!("Directory '{}' already exists", name));
    }
    
    fs::create_dir_all(&project_path)?;
    println!("   ✓ Created project directory: {}", name.bright_green());
    
    // Create atomo directory structure
    let atomo_dir = project_path.join("atomo");
    fs::create_dir_all(&atomo_dir)?;
    
    // Create schema.ts based on template or default
    let schema_content = match template.as_deref() {
        Some("crm") => include_str!("../../templates/crm/schema.ts"),
        Some("blog") => include_str!("../../templates/blog/schema.ts"),
        Some("ecommerce") => include_str!("../../templates/ecommerce/schema.ts"),
        _ => include_str!("../../templates/default/schema.ts"),
    };
    
    fs::write(atomo_dir.join("schema.ts"), schema_content)?;
    println!("   ✓ Created schema definition");
    
    // Create package.json
    let package_json = serde_json::json!({
        "name": name,
        "version": "0.1.0",
        "description": "An Atomo Content Core project",
        "scripts": {
            "dev": "atomo dev",
            "build": "atomo build",
            "atomo:generate": "atomo generate",
            "atomo:migrate": "atomo migrate"
        },
        "dependencies": {
            "@atomo/client": "^0.1.0"
        },
        "devDependencies": {
            "typescript": "^5.0.0"
        }
    });
    
    fs::write(
        project_path.join("package.json"),
        serde_json::to_string_pretty(&package_json)?
    )?;
    println!("   ✓ Created package.json");
    
    // Create README.md
    let readme_content = format!(r#"# {}

A new Atomo Content Core project.

## Getting Started

1. Install dependencies:
   ```bash
   npm install
   ```

2. Generate Rust code from schema:
   ```bash
   npm run atomo:generate
   ```

3. Run database migrations:
   ```bash
   npm run atomo:migrate
   ```

4. Start development server:
   ```bash
   npm run dev
   ```

## Project Structure

- `atomo/schema.ts` - Your content model definitions
- `generated/` - Auto-generated client code
- `src/` - Your application code

## Documentation

Visit [atomo.cc/docs](https://atomo.cc/docs) for full documentation.
"#, name);
    
    fs::write(project_path.join("README.md"), readme_content)?;
    println!("   ✓ Created README.md");
    
    println!();
    println!("🎉 {}", "Project initialized successfully!".bright_green().bold());
    println!();
    println!("Next steps:");
    println!("  cd {}", name.bright_cyan());
    println!("  npm install");
    println!("  npm run atomo:generate");
    println!("  npm run dev");
    
    Ok(())
}
