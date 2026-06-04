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

    fs::create_dir_all(project_path)?;
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
            "@atomo-cc/client-sdk": "^0.1.0"
        },
        "devDependencies": {
            "typescript": "^5.0.0"
        }
    });

    fs::write(
        project_path.join("package.json"),
        serde_json::to_string_pretty(&package_json)?,
    )?;
    println!("   ✓ Created package.json");

    // Create README.md
    let readme_content = format!(
        r#"# {}

A new Atomo Content Core project.

## Run it (no Rust required)

The fastest way: Docker pulls the prebuilt server image and runs it against your
schema, alongside a Postgres database. No Rust, no toolchain to install.

```bash
docker compose up                    # http://localhost:3000
curl http://localhost:3000/health    # -> OK
```

The image bundles the **Admin UI** at <http://localhost:3000/admin>. Your data
model lives in `atomo/schema.ts`; edit it and re-run `docker compose up` to apply
changes — the server re-parses the schema and runs migrations on boot.

## Develop with the CLI (optional, needs Rust)

If you have the Atomo CLI installed, you also get schema hot-reload:

```bash
npm install
npm run atomo:generate   # generate typed client SDK from the schema
npm run dev              # atomo dev — hot reload (requires the Rust toolchain)
```

## Project Structure

- `atomo/schema.ts` - Your content model definitions
- `docker-compose.yml` - Run the server + Postgres with no Rust
- `generated/` - Auto-generated client code
- `src/` - Your application code

## Documentation

Visit [atomo.cc/docs](https://atomo.cc/docs) for full documentation.
"#,
        name
    );

    fs::write(project_path.join("README.md"), readme_content)?;
    println!("   ✓ Created README.md");

    // Create docker-compose.yml — the zero-Rust run path. Pulls the published
    // server image and mounts this project's schema; `docker compose up` runs it.
    let compose_content = r#"# Run this Atomo project with no Rust toolchain: `docker compose up`.
# Pulls the prebuilt atomo-server image and points it at ./atomo/schema.ts.
services:
  db:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: atomo
      POSTGRES_PASSWORD: atomo
      POSTGRES_DB: atomo_dev
    volumes:
      - atomo-db:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U atomo -d atomo_dev"]
      interval: 5s
      timeout: 3s
      retries: 12

  server:
    image: ghcr.io/chris533/atomo-server:latest
    depends_on:
      db:
        condition: service_healthy
    environment:
      DATABASE_URL: postgresql://atomo:atomo@db:5432/atomo_dev
      ATOMO_SCHEMA_PATH: /app/atomo/schema.ts
      # Dev-only secret/credentials — override for anything real.
      JWT_SECRET: dev-insecure-secret-change-me
      ADMIN_EMAIL: admin@example.com
      ADMIN_PASSWORD: change-me-too
      PORT: "3000"
    ports:
      - "3000:3000"
    volumes:
      - ./atomo/schema.ts:/app/atomo/schema.ts:ro

volumes:
  atomo-db:
"#;

    fs::write(project_path.join("docker-compose.yml"), compose_content)?;
    println!("   ✓ Created docker-compose.yml (run with no Rust: docker compose up)");

    println!();
    println!(
        "🎉 {}",
        "Project initialized successfully!".bright_green().bold()
    );
    println!();
    println!("Next steps:");
    println!("  cd {}", name.bright_cyan());
    println!("  {}            # run with no Rust (Docker)", "docker compose up".bright_cyan());
    println!();
    println!("  …or develop with the CLI (needs Rust):");
    println!("  npm install && npm run atomo:generate && npm run dev");

    Ok(())
}
