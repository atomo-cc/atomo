//! Example of using Atomo as a library
//!
//! This demonstrates the new library API that makes Atomo work like Prisma,
//! Strapi, or other popular frameworks where you use it as a dependency
//! rather than just a CLI tool.

use atomo::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Example 1: Initialize Atomo from schema file (Prisma-like)
    println!("🚀 Initializing Atomo from schema file...");
    
    let atomo = Atomo::from_schema("./packages/atomo-crm-app/atomo/schema.ts")
        .await
        .expect("Failed to initialize Atomo");
    
    println!("✅ Atomo initialized successfully!");
    
    // Example 2: Type-safe queries with fluent API
    println!("\n📊 Performing type-safe queries...");
    
    // Find all contacts with fluent API
    let contacts = atomo
        .contact()
        .find_many()
        .where_("name", Contains("John".to_string()))
        .order_by("created_at", Desc)
        .limit(10)
        .exec()
        .await;
    
    match contacts {
        Ok(results) => println!("Found {} contacts", results.len()),
        Err(e) => println!("Query failed: {}", e),
    }
    
    // Example 3: Creating records
    println!("\n➕ Creating new contact...");
    
    let new_contact = atomo
        .contact()
        .create()
        .set("name", "Jane Doe")
        .set("email", "jane@example.com")
        .set("phone", "+1-555-0123")
        .exec()
        .await;
    
    match new_contact {
        Ok(contact) => println!("Created contact: {:?}", contact),
        Err(e) => println!("Creation failed: {}", e),
    }
    
    // Example 4: Real-time subscriptions (Event Sourcing)
    println!("\n🔄 Setting up real-time subscriptions...");
    
    let mut contact_stream = atomo
        .contact()
        .subscribe()
        .on_create()
        .stream()
        .await;
    
    println!("Listening for new contacts... (this would run forever in a real app)");
    
    // In a real application, you'd listen like this:
    // while let Some(event) = contact_stream.next().await {
    //     println!("New contact created: {:?}", event.data);
    // }
    
    // Example 5: Builder pattern for complex setups
    println!("\n⚙️  Using builder pattern for advanced configuration...");
    
    let atomo_advanced = Atomo::builder()
        .database_url("postgresql://localhost/atomo_dev")
        .schema_file("./packages/atomo-crm-app/atomo/schema.ts")
        .enable_migrations(true)
        .enable_ai(false)
        .build()
        .await;
    
    match atomo_advanced {
        Ok(_) => println!("✅ Advanced Atomo configuration successful!"),
        Err(e) => println!("⚠️  Advanced setup failed (expected): {}", e),
    }
    
    // Example 6: GraphQL schema generation for integration
    println!("\n🔗 Generating GraphQL schema for API integration...");
    
    let graphql_schema = atomo.graphql_schema();
    println!("✅ GraphQL schema generated - ready for API server!");
    
    println!("\n🎉 All examples completed!");
    println!("💡 Atomo can now be used as a library, just like Prisma, Strapi, or Payload!");
    
    Ok(())
}
