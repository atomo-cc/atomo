//! Integration test for Hook and Access Control DSL parsing and code generation
//!
//! This test demonstrates the complete flow from TypeScript DSL to Rust execution code.

#[cfg(test)]
mod tests {
    use crate::{AccessRule, HookAccessGenerator, QueryValue, TypeScriptParser};
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

        // Debug: Print what we found
        println!("Found {} models", models.len());
        for model in &models {
            println!("Model: {}", model.name);
            println!("  Fields: {:?}", model.fields.keys().collect::<Vec<_>>());
            println!("  Has access: {}", model.access.is_some());
            println!("  Has hooks: {}", model.hooks.is_some());
        }

        // 3. Verify models were parsed correctly
        assert!(!models.is_empty(), "Should parse at least one model");

        let product_model = models
            .iter()
            .find(|m| m.name == "Product")
            .expect("Should find Product model");
        assert!(
            product_model.fields.contains_key("title"),
            "Should have title field"
        );
        assert!(
            product_model.fields.contains_key("price"),
            "Should have price field"
        );

        // 4. Verify access control was parsed
        if let Some(access) = &product_model.access {
            println!("Access control found:");
            println!("  create: {:?}", access.create);
            println!("  read: {:?}", access.read);
            println!("  update: {:?}", access.update);
            println!("  delete: {:?}", access.delete);

            assert!(access.create.is_some(), "Should have create access rule");
            assert!(access.read.is_some(), "Should have read access rule");
            assert!(access.update.is_some(), "Should have update access rule");
            assert!(access.delete.is_some(), "Should have delete access rule");

            // Check create rule (simple boolean)
            if let Some(AccessRule::Boolean(code)) = &access.create {
                assert_eq!(code, "!!user", "Create rule should check user existence");
            } else {
                panic!(
                    "Expected Boolean access rule for create, got: {:?}",
                    access.create
                );
            }

            // Check update rule (query)
            if let Some(AccessRule::Query(condition)) = &access.update {
                assert_eq!(
                    condition.field, "sellerId",
                    "Update rule should check sellerId"
                );
                assert!(
                    matches!(condition.value, QueryValue::UserProperty(_)),
                    "Should reference user property"
                );
            }
        } else {
            panic!("Product model should have access control defined");
        }

        // 5. Verify hooks were parsed
        if let Some(hooks) = &product_model.hooks {
            println!("Hooks found:");
            println!(
                "  before_operation: {:?}",
                hooks.before_operation.as_ref().map(|h| h.len())
            );
            println!(
                "  after_operation: {:?}",
                hooks.after_operation.as_ref().map(|h| h.len())
            );
            println!(
                "  before_change: {:?}",
                hooks.before_change.as_ref().map(|h| h.len())
            );
            println!(
                "  after_read: {:?}",
                hooks.after_read.as_ref().map(|h| h.len())
            );

            assert!(
                hooks.before_operation.is_some(),
                "Should have beforeOperation hooks"
            );
            assert!(
                hooks.after_operation.is_some(),
                "Should have afterOperation hooks"
            );
            assert!(
                hooks.before_change.is_some(),
                "Should have beforeChange hooks"
            );
            assert!(hooks.after_read.is_some(), "Should have afterRead hooks");

            // Check beforeOperation hooks
            if let Some(before_ops) = &hooks.before_operation {
                assert!(
                    !before_ops.is_empty(),
                    "Should have at least one beforeOperation hook"
                );
                let first_hook = &before_ops[0];
                assert_eq!(
                    first_hook.name, "create",
                    "First hook should be create hook"
                );
                assert!(first_hook.async_hook, "Hook should be async");
            }

            // Check beforeChange hooks
            if let Some(before_changes) = &hooks.before_change {
                assert!(
                    !before_changes.is_empty(),
                    "Should have at least one beforeChange hook"
                );
                let field_hook = &before_changes[0];
                assert_eq!(
                    field_hook.field_name, "status",
                    "Field hook should be for status field"
                );
            }
        } else {
            panic!("Product model should have hooks defined");
        }

        // 6. Generate Rust code
        let generator = HookAccessGenerator::new();
        let rust_code = generator.generate_module(&models)?;

        // 7. Verify generated code contains expected structures
        assert!(
            rust_code.contains("AccessContext"),
            "Should generate AccessContext struct"
        );
        assert!(
            rust_code.contains("HookContext"),
            "Should generate HookContext struct"
        );
        assert!(
            rust_code.contains("ProductAccessControl"),
            "Should generate ProductAccessControl"
        );
        assert!(
            rust_code.contains("ProductHooks"),
            "Should generate ProductHooks"
        );

        // 8. Verify some generated methods exist
        assert!(
            rust_code.contains("check_create"),
            "Should generate check_create method"
        );
        assert!(
            rust_code.contains("before_operation"),
            "Should generate hook methods"
        );

        println!("✅ Complete Hook and Access Control DSL flow test passed!");
        println!("📊 Parsed {} models", models.len());
        println!(
            "🔧 Generated {} lines of Rust code",
            rust_code.lines().count()
        );
        println!("🎯 Access Control:");
        println!("   - create: Boolean check");
        println!("   - read: Query conditions");
        println!("   - update/delete: User ownership checks");
        println!("🪝 Hooks:");
        println!(
            "   - beforeOperation: {} hooks",
            product_model
                .hooks
                .as_ref()
                .unwrap()
                .before_operation
                .as_ref()
                .unwrap()
                .len()
        );
        println!(
            "   - afterOperation: {} hooks",
            product_model
                .hooks
                .as_ref()
                .unwrap()
                .after_operation
                .as_ref()
                .unwrap()
                .len()
        );
        println!(
            "   - beforeChange: {} hooks",
            product_model
                .hooks
                .as_ref()
                .unwrap()
                .before_change
                .as_ref()
                .unwrap()
                .len()
        );
        println!(
            "   - afterRead: {} hooks",
            product_model
                .hooks
                .as_ref()
                .unwrap()
                .after_read
                .as_ref()
                .unwrap()
                .len()
        );

        Ok(())
    }

    #[test]
    fn test_basic_model_parsing() {
        let schema_content = r#"
export interface User {
  id: string;
  name: string;
  email: string;
}
"#;

        let parser = TypeScriptParser::new();
        let models = parser.parse(schema_content).unwrap();

        assert!(!models.is_empty(), "Should parse at least one model");
        let user_model = models
            .iter()
            .find(|m| m.name == "User")
            .expect("Should find User model");
        assert!(user_model.fields.contains_key("id"), "Should have id field");
        assert!(
            user_model.fields.contains_key("name"),
            "Should have name field"
        );
        assert!(
            user_model.fields.contains_key("email"),
            "Should have email field"
        );
    }

    #[test]
    fn test_hook_access_generator_basic() {
        let generator = HookAccessGenerator::new();

        // Test that we can create the generator
        let result = generator.generate_module(&[]);
        assert!(result.is_ok(), "Should be able to generate empty module");

        let code = result.unwrap();
        assert!(
            code.contains("AccessContext"),
            "Should generate AccessContext struct"
        );
        assert!(
            code.contains("HookContext"),
            "Should generate HookContext struct"
        );
    }
}
