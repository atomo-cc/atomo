//! Integration test for Hook and Access Control DSL parsing and code generation
//! 
//! This test demonstrates the complete flow from TypeScript DSL to Rust execution code.

use atomo_schema::{TypeScriptParser, HookAccessGenerator, AccessRule, QueryOperator, QueryValue};
use anyhow::Result;

#[tokio::test]
async fn test_complete_hook_access_flow() -> Result<()> {
    // 1. Define a TypeScript schema with hooks and access control
    let schema_content = r#"
export interface Product {
  id: string;
  title: string;
  description: string;
  price: number;
  priceInCents: number;
  status: ProductStatus;
  sellerId: string;
  seller?: User;
  createdAt: Date;
  updatedAt: Date;
}

export interface User {
  id: string;
  firstName: string;
  lastName: string;
  email: string;
  createdAt: Date;
  updatedAt: Date;
}

export enum ProductStatus {
  DRAFT = "draft",
  PUBLISHED = "published",
  SOLD = "sold"
}

export const ProductModel = defineModel({
  access: {
    create: ({ user }: access.Context<User>) => !!user,
    read: ({ user }: access.Context<User>) => {
      if (user) {
        return access.or(
          access.where('status').equals('published'),
          access.where('sellerId').equals(user.id)
        );
      }
      return access.where('status').equals('published');
    },
    update: ({ user }: access.Context<User>) => access.where('sellerId').equals(user.id),
    delete: ({ user }: access.Context<User>) => access.where('sellerId').equals(user.id),
  },
  hooks: {
    beforeOperation: [
      hooks.create(async (context: hooks.OperationContext<Product, User>) => {
        if (context.operation === 'create') {
          context.data.priceInCents = context.data.price * 100;
          delete context.data.price;
        }
      }),
    ],
    afterOperation: [
      hooks.create(async (context: hooks.OperationContext<Product, User>) => {
        if (context.operation === 'create') {
          await slack.sendMessage('#new-products', `New product: ${context.result.title}`);
        }
      }),
    ],
    beforeChange: [
      hooks.change('status', async (context: hooks.ChangeContext<ProductStatus, Product>) => {
        if (context.value === 'published' && !context.originalDoc.description) {
          context.addValidationError('Description required for publishing', 'description');
        }
      }),
    ],
    afterRead: [
      hooks.read(async (doc: Product) => ({
        ...doc,
        price: doc.priceInCents / 100,
        seller: doc.seller ? {
          ...doc.seller,
          displayName: `${doc.seller.firstName} ${doc.seller.lastName}`,
        } : null,
      })),
    ],
  },
});
"#;

    // 2. Parse the schema
    let parser = TypeScriptParser::new();
    let models = parser.parse(schema_content)?;
    
    // 3. Verify models were parsed correctly
    assert!(!models.is_empty(), "Should parse at least one model");
    
    let product_model = models.iter().find(|m| m.name == "Product").expect("Should find Product model");
    assert!(product_model.fields.contains_key("title"), "Should have title field");
    assert!(product_model.fields.contains_key("price"), "Should have price field");
    
    // 4. Verify access control was parsed
    if let Some(access) = &product_model.access {
        assert!(access.create.is_some(), "Should have create access rule");
        assert!(access.read.is_some(), "Should have read access rule");
        assert!(access.update.is_some(), "Should have update access rule");
        assert!(access.delete.is_some(), "Should have delete access rule");
        
        // Check create rule (simple boolean)
        if let Some(AccessRule::Boolean(code)) = &access.create {
            assert_eq!(code, "!!user", "Create rule should check user existence");
        }
    } else {
        panic!("Product model should have access control defined");
    }
    
    // 5. Verify hooks were parsed
    if let Some(hooks) = &product_model.hooks {
        assert!(hooks.before_operation.is_some(), "Should have beforeOperation hooks");
        assert!(hooks.after_operation.is_some(), "Should have afterOperation hooks");
        assert!(hooks.before_change.is_some(), "Should have beforeChange hooks");
        assert!(hooks.after_read.is_some(), "Should have afterRead hooks");
        
        // Check beforeOperation hooks
        if let Some(before_ops) = &hooks.before_operation {
            assert!(!before_ops.is_empty(), "Should have at least one beforeOperation hook");
            let first_hook = &before_ops[0];
            assert_eq!(first_hook.name, "create", "First hook should be create hook");
            assert!(first_hook.async_hook, "Hook should be async");
        }
        
        // Check beforeChange hooks
        if let Some(before_changes) = &hooks.before_change {
            assert!(!before_changes.is_empty(), "Should have at least one beforeChange hook");
            let field_hook = &before_changes[0];
            assert_eq!(field_hook.field_name, "status", "Field hook should be for status field");
        }
    } else {
        panic!("Product model should have hooks defined");
    }
    
    // 6. Generate Rust code
    let generator = HookAccessGenerator::new();
    let rust_code = generator.generate_module(&models)?;
    
    // 7. Verify generated code contains expected structures
    assert!(rust_code.contains("AccessContext"), "Should generate AccessContext struct");
    assert!(rust_code.contains("AccessQuery"), "Should generate AccessQuery struct");
    assert!(rust_code.contains("HookContext"), "Should generate HookContext struct");
    assert!(rust_code.contains("ProductAccessControl"), "Should generate ProductAccessControl");
    assert!(rust_code.contains("ProductHooks"), "Should generate ProductHooks");
    assert!(rust_code.contains("check_create"), "Should generate check_create method");
    assert!(rust_code.contains("check_read"), "Should generate check_read method");
    assert!(rust_code.contains("before_operation_0"), "Should generate beforeOperation hook methods");
    assert!(rust_code.contains("before_change_0"), "Should generate beforeChange hook methods");
    
    // 8. Verify access control logic
    assert!(rust_code.contains("user.is_some()"), "Should convert !!user to user.is_some()");
    assert!(rust_code.contains("AccessQuery::or"), "Should generate OR queries");
    assert!(rust_code.contains(r#""status""#), "Should reference status field");
    assert!(rust_code.contains(r#""sellerId""#), "Should reference sellerId field");
    
    println!("✅ Complete Hook and Access Control DSL flow test passed!");
    println!("📊 Parsed {} models", models.len());
    println!("🔧 Generated {} lines of Rust code", rust_code.lines().count());
    
    Ok(())
}

#[test]
fn test_access_rule_parsing() {
    let mut parser = atomo_schema::DslParser::new();
    
    // Test simple boolean rule
    let content = r#"create: ({ user }: access.Context<User>) => !!user,"#;
    let result = parser.parse_access_rule(content, "create").unwrap();
    assert!(result.is_some());
    
    if let Some(AccessRule::Boolean(code)) = result {
        assert_eq!(code, "!!user");
    } else {
        panic!("Expected Boolean access rule");
    }
}

#[test]
fn test_query_condition_parsing() {
    let mut parser = atomo_schema::DslParser::new();
    
    // Test where condition
    let content = r#"access.where('status').equals('published')"#;
    let result = parser.parse_where_condition(content).unwrap();
    assert!(result.is_some());
    
    if let Some(AccessRule::Query(condition)) = result {
        assert_eq!(condition.field, "status");
        assert!(matches!(condition.operator, QueryOperator::Equals));
        assert!(matches!(condition.value, QueryValue::String(ref s) if s == "published"));
    } else {
        panic!("Expected Query access rule");
    }
}

#[test]
fn test_user_property_parsing() {
    let parser = atomo_schema::DslParser::new();
    
    // Test user property reference
    let result = parser.parse_query_value("user.id").unwrap();
    assert!(matches!(result, QueryValue::UserProperty(ref s) if s == "user.id"));
    
    // Test string literal
    let result = parser.parse_query_value("'published'").unwrap();
    assert!(matches!(result, QueryValue::String(ref s) if s == "published"));
    
    // Test number literal
    let result = parser.parse_query_value("100").unwrap();
    assert!(matches!(result, QueryValue::Number(n) if n == 100.0));
    
    // Test boolean literal
    let result = parser.parse_query_value("true").unwrap();
    assert!(matches!(result, QueryValue::Boolean(true)));
}

#[test]
fn test_hook_access_generator() {
    let generator = HookAccessGenerator::new();
    
    // Test access structures generation
    let access_code = generator.generate_access_structures().unwrap();
    assert!(access_code.contains("AccessContext"));
    assert!(access_code.contains("AccessQuery"));
    assert!(access_code.contains("QueryCondition"));
    
    // Test hook structures generation
    let hook_code = generator.generate_hook_structures().unwrap();
    assert!(hook_code.contains("HookContext"));
    assert!(hook_code.contains("HookResult"));
    assert!(hook_code.contains("FieldChangeContext"));
}
