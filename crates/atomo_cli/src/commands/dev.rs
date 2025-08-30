use anyhow::{Result, Context};
use colored::*;
use console::style;
use std::net::SocketAddr;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use async_graphql::{Schema, EmptyMutation, EmptySubscription, Object, FieldResult, ID};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use atomo_schema::{TypeScriptParser, GraphQLGenerator, ResolverGenerator};

#[derive(Clone)]
struct AppState {
    db: PgPool,
    graphql_schema: Schema<PlatformQuery, EmptyMutation, EmptySubscription>,
}

// GraphQL Query root - Basic platform queries
struct Query;

#[Object]
impl Query {
    /// API status and information
    async fn status(&self) -> String {
        "Atomo Platform is running".to_string()
    }

    /// Get platform version
    async fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

// Platform Query root - Pure platform functionality, no business logic
struct PlatformQuery;

#[Object]
impl PlatformQuery {
    /// Platform status and information
    async fn status(&self) -> String {
        "Atomo Content Core Platform".to_string()
    }

    /// Get platform version
    async fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Show platform capabilities
    async fn capabilities(&self) -> Vec<String> {
        vec![
            "Dual-Mode Schema (schema.ts -> Rust)".to_string(),
            "Dynamic GraphQL API Generation".to_string(),
            "Type-Safe Code Generation".to_string(),
            "PostgreSQL Integration".to_string(),
            "Real-time Development Server".to_string(),
        ]
    }
}

pub async fn dev_command(port: u16) -> Result<()> {
    println!("🚀 {}", style("Starting Atomo development server...").cyan());
    
    // Load environment variables
    let env_loaded = dotenv::dotenv().is_ok();
    if env_loaded {
        println!("   ✅ Environment variables loaded");
    }
    
    // Phase 1: Implement true "Dual-Mode Schema" system
    let mut has_generated_resolvers = false;
    
    let current_dir = std::env::current_dir()
        .with_context(|| "Failed to get current directory")?;
    let schema_path = current_dir.join("schema.ts");
    if schema_path.exists() {
        println!("   📋 Found schema.ts, implementing dual-mode schema...");
        
        let schema_content = std::fs::read_to_string(schema_path)
            .with_context(|| "Failed to read schema.ts")?;
        
        let parser = TypeScriptParser::new();
        match parser.parse(&schema_content) {
            Ok(models) => {
                println!("   ✅ Parsed {} models from schema", models.len());
                
                // Step 1: Generate GraphQL SDL
                let generator = GraphQLGenerator::new();
                match generator.generate_schema(&models) {
                    Ok(_schema_sdl) => {
                        println!("   ✅ Generated GraphQL schema");
                    }
                    Err(e) => {
                        println!("   ⚠️  Warning: Failed to generate GraphQL schema: {}", e);
                    }
                }
                
                // Step 2: Auto-run atomo generate to ensure fresh resolvers
                println!("   🔄 Auto-generating Rust resolvers...");
                let resolver_generator = ResolverGenerator::new();
                match resolver_generator.generate_resolvers(&models) {
                    Ok(resolver_code) => {
                        // Ensure generated directory exists
                        std::fs::create_dir_all("generated").ok();
                        
                        // Write fresh resolvers.rs
                        std::fs::write("generated/resolvers.rs", resolver_code)
                            .with_context(|| "Failed to write generated resolvers")?;
                        
                        // Write a simple mod.rs to make it a module
                        std::fs::write("generated/mod.rs", "pub mod resolvers;\npub use resolvers::*;\n")
                            .with_context(|| "Failed to write generated mod.rs")?;
                        
                        println!("   ✅ Generated fresh Rust resolvers");
                        has_generated_resolvers = true;
                    }
                    Err(e) => {
                        println!("   ⚠️  Warning: Failed to generate resolvers: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("   ⚠️  Warning: Failed to parse schema.ts: {}", e);
                println!("      Using basic platform schema");
            }
        }
    } else {
        println!("   📋 No schema.ts found, using basic platform schema");
    }
    
    // Get database URL
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/atomo_dev".to_string());
    
    println!("   📊 Connecting to database...");
    
    // Connect to database
    let db = PgPool::connect(&db_url).await
        .with_context(|| "Failed to connect to database. Run 'atomo migrate' first")?;
    
    println!("   ✅ Database connected");
    
    // Phase 1: Create GraphQL schema - Pure platform with dynamic generated resolvers
    let schema = if has_generated_resolvers {
        println!("   🎯 Creating dynamic GraphQL schema from generated resolvers");
        
        // TODO: This is where we need to dynamically load and merge generated resolvers
        // For now, we'll use the platform schema and note that generated resolvers are available
        println!("   📝 Note: Generated resolvers available at: generated/resolvers.rs");
        println!("   📝 Next: Implement dynamic resolver loading for true dual-mode schema");
        
        Schema::build(PlatformQuery, EmptyMutation, EmptySubscription)
            .data(db.clone())
            .finish()
    } else {
        println!("   📚 Creating basic platform GraphQL schema");
        Schema::build(PlatformQuery, EmptyMutation, EmptySubscription)
            .data(db.clone())
            .finish()
    };
    
    let state = AppState { 
        db,
        graphql_schema: schema,
    };
    
    // Build the application router
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/health", get(health_handler))
        .route("/graphql", post(graphql_handler).get(graphql_playground_handler))
        .route("/admin", get(admin_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);
    
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    
    println!("   🌐 Starting server on port {}...", port);
    println!();
    println!("🎉 {}", "Atomo Development Server is running!".bright_green().bold());
    println!();
    println!("   🏠 Homepage:           {}", format!("http://localhost:{}", port).bright_blue());
    println!("   🔍 GraphQL Playground: {}", format!("http://localhost:{}/graphql", port).bright_blue());
    println!("   ⚙️  Admin Interface:   {}", format!("http://localhost:{}/admin", port).bright_blue());
    println!("   💚 Health Check:      {}", format!("http://localhost:{}/health", port).bright_blue());
    println!();
    println!("Press Ctrl+C to stop the server");
    println!();
    
    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await
        .with_context(|| format!("Failed to bind to address {}", addr))?;
    
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .with_context(|| "Server error")?;
    
    println!("\n👋 {}", "Server stopped gracefully".bright_yellow());
    
    Ok(())
}

async fn index_handler() -> impl IntoResponse {
    Html(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Atomo Development Server</title>
    <style>
        body { 
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 800px; 
            margin: 50px auto; 
            padding: 20px;
            background: #f5f5f5;
            color: #333;
        }
        .header { 
            text-align: center; 
            margin-bottom: 40px;
        }
        .logo {
            font-size: 3em;
            font-weight: bold;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            margin-bottom: 10px;
        }
        .tagline {
            color: #666;
            font-size: 1.2em;
        }
        .cards {
            display: grid;
            gap: 20px;
            margin-top: 40px;
        }
        .card {
            background: white;
            padding: 20px;
            border-radius: 10px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            text-decoration: none;
            color: inherit;
            transition: transform 0.2s;
        }
        .card:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 20px rgba(0,0,0,0.15);
        }
        .card-title {
            font-size: 1.3em;
            font-weight: bold;
            margin-bottom: 10px;
            color: #667eea;
        }
        .card-description {
            color: #666;
            line-height: 1.5;
        }
        .status {
            background: #e8f5e8;
            border: 1px solid #4caf50;
            border-radius: 5px;
            padding: 10px;
            margin: 20px 0;
            text-align: center;
            color: #2e7d32;
        }
    </style>
</head>
<body>
    <div class="header">
        <div class="logo">⚛️ Atomo</div>
        <div class="tagline">The Next-Generation Content Core</div>
    </div>
    
    <div class="status">
        🟢 Development server is running
    </div>
    
    <div class="cards">
        <a href="/graphql" class="card">
            <div class="card-title">🔍 GraphQL Playground</div>
            <div class="card-description">
                Interactive GraphQL API explorer. Query your data, explore the schema, and test mutations.
            </div>
        </a>
        
        <a href="/admin" class="card">
            <div class="card-title">⚙️ Admin Interface</div>
            <div class="card-description">
                Manage your content, view data models, and configure your Atomo instance.
            </div>
        </a>
        
        <a href="/health" class="card">
            <div class="card-title">💚 Health Check</div>
            <div class="card-description">
                Check the status of your database connection and server health.
            </div>
        </a>
    </div>
    
    <div style="text-align: center; margin-top: 40px; color: #999;">
        Built with ❤️ using Rust + Axum
    </div>
</body>
</html>
    "#)
}

async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    // Check database connection
    let db_status = match sqlx::query("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => "✅ Connected",
        Err(_) => "❌ Disconnected",
    };
    
    let health_info = format!(
        r#"{{
    "status": "ok",
    "database": "{}",
    "timestamp": "{}",
    "version": "0.1.0"
}}"#,
        db_status,
        chrono::Utc::now().to_rfc3339()
    );
    
    (StatusCode::OK, health_info)
}

async fn graphql_handler(
    State(state): State<AppState>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    state.graphql_schema.execute(req.into_inner()).await.into()
}

async fn graphql_playground_handler() -> impl IntoResponse {
    Html(async_graphql::http::playground_source(
        async_graphql::http::GraphQLPlaygroundConfig::new("/graphql"),
    ))
}

async fn admin_handler() -> impl IntoResponse {
    Html(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Atomo Admin</title>
    <style>
        body { 
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            background: #f8fafc;
        }
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 20px;
            text-align: center;
        }
        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 40px 20px;
        }
        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin-top: 30px;
        }
        .card {
            background: white;
            border-radius: 10px;
            padding: 20px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }
        .card h3 {
            margin-top: 0;
            color: #667eea;
        }
        .badge {
            background: #e3f2fd;
            color: #1976d2;
            padding: 4px 8px;
            border-radius: 4px;
            font-size: 0.8em;
            font-weight: bold;
        }
        .coming-soon {
            background: #fff3e0;
            color: #f57c00;
        }
    </style>
</head>
<body>
    <div class="header">
        <h1>⚛️ Atomo Admin</h1>
        <p>Content Core Administration Interface</p>
    </div>
    
    <div class="container">
        <div class="grid">
            <div class="card">
                <h3>📊 Models</h3>
                <p>Manage your content models and schemas</p>
                <span class="badge coming-soon">Coming Soon</span>
            </div>
            
            <div class="card">
                <h3>📝 Content</h3>
                <p>Create and edit your content entries</p>
                <span class="badge coming-soon">Coming Soon</span>
            </div>
            
            <div class="card">
                <h3>👥 Users</h3>
                <p>Manage user accounts and permissions</p>
                <span class="badge coming-soon">Coming Soon</span>
            </div>
            
            <div class="card">
                <h3>� Settings</h3>
                <p>Configure your Atomo instance</p>
                <span class="badge coming-soon">Coming Soon</span>
            </div>
            
            <div class="card">
                <h3>📈 Analytics</h3>
                <p>View usage statistics and insights</p>
                <span class="badge coming-soon">Coming Soon</span>
            </div>
            
            <div class="card">
                <h3>🔌 Plugins</h3>
                <p>Manage WASM plugins and extensions</p>
                <span class="badge coming-soon">Coming Soon</span>
            </div>
        </div>
    </div>
</body>
</html>
    "#)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
