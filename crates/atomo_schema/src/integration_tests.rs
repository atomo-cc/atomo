//! Integration test for Hook and Access Control DSL parsing and code generation
//!
//! This test demonstrates the complete flow from TypeScript DSL to Rust execution code.

#[cfg(test)]
mod tests {
    use crate::{AccessRule, HookAccessGenerator, QueryValue, TypeScriptParser};
    use anyhow::Result;

    fn crm_schema_content() -> String {
        let base =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../services/crm-service");
        let schema = std::fs::read_to_string(base.join("atomo/schema.ts"))
            .expect("CRM schema.ts should exist");
        let actions = std::fs::read_to_string(base.join("atomo/actions.ts"))
            .expect("CRM actions.ts should exist");
        // Concatenate so the parser sees action definitions alongside the schema
        format!("{actions}\n\n{schema}")
    }

    #[test]
    fn crm_schema_events_and_actions_parse_correctly() {
        let content = crm_schema_content();
        assert!(
            crate::schema_dsl_parser::is_builder_dsl(&content),
            "CRM schema should be detected as builder DSL"
        );
        let schema = crate::schema_dsl_parser::parse_builder_dsl(&content).unwrap();

        // All 6 models present
        assert!(schema.models.contains_key("User"));
        assert!(schema.models.contains_key("Company"));
        assert!(schema.models.contains_key("Contact"));
        assert!(schema.models.contains_key("Lead"));
        assert!(schema.models.contains_key("Deal"));
        assert!(schema.models.contains_key("Activity"));

        // User.created → sendWelcomeEmail
        let user = &schema.models["User"];
        assert_eq!(
            user.events.created.len(),
            1,
            "User should have 1 created event"
        );
        assert_eq!(user.events.created[0].action, "sendWelcomeEmail");
        assert!(user.events.created[0].condition.is_none());

        // Company.created → enrichCompany
        let company = &schema.models["Company"];
        assert_eq!(
            company.events.created.len(),
            1,
            "Company should have 1 created event"
        );
        assert_eq!(company.events.created[0].action, "enrichCompany");

        // Company.updated → enrichCompany.whenChanged('website', ...)
        assert_eq!(company.events.updated.len(), 1);
        assert_eq!(company.events.updated[0].action, "enrichCompany");
        assert!(company.events.updated[0].condition.is_some());

        // Lead.created → scoreLead + rollupLeadStats
        let lead = &schema.models["Lead"];
        assert_eq!(
            lead.events.created.len(),
            2,
            "Lead should have 2 created events"
        );
        let lead_created_names: Vec<&str> = lead
            .events
            .created
            .iter()
            .map(|e| e.action.as_str())
            .collect();
        assert!(lead_created_names.contains(&"scoreLead"));
        assert!(lead_created_names.contains(&"rollupLeadStats"));

        // Lead.updated has conditional actions
        assert_eq!(
            lead.events.updated.len(),
            2,
            "Lead should have 2 updated events"
        );

        // Activity.created → updateContactLastActivity
        let activity = &schema.models["Activity"];
        assert_eq!(activity.events.created.len(), 1);
        assert_eq!(
            activity.events.created[0].action,
            "updateContactLastActivity"
        );

        // Lifecycle actions parsed (at least the .from().input([]) ones)
        let welcome = schema
            .actions
            .get("sendWelcomeEmail")
            .expect("sendWelcomeEmail action");
        assert_eq!(welcome.source_model.as_deref(), Some("User"));
        if let crate::types::ActionInputDef::PickFields { model, fields } = &welcome.input {
            assert_eq!(model, "User");
            assert!(fields.contains(&"id".to_string()));
            assert!(fields.contains(&"email".to_string()));
        } else {
            panic!("expected PickFields for sendWelcomeEmail");
        }

        let enrich = schema
            .actions
            .get("enrichCompany")
            .expect("enrichCompany action");
        assert_eq!(enrich.source_model.as_deref(), Some("Company"));

        let score = schema.actions.get("scoreLead").expect("scoreLead action");
        assert_eq!(score.source_model.as_deref(), Some("Lead"));
    }

    #[test]
    fn crm_codegen_matches_committed_file() {
        let content = crm_schema_content();
        let schema = if crate::schema_dsl_parser::is_builder_dsl(&content) {
            crate::schema_dsl_parser::parse_builder_dsl(&content).unwrap()
        } else {
            TypeScriptParser::new().parse_schema(&content).unwrap()
        };
        let generated = crate::codegen::generate_typescript_client(&schema);

        let committed_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../services/crm-service/generated/client.ts");
        let committed = std::fs::read_to_string(&committed_path)
            .expect("committed generated/client.ts should exist");

        if generated.trim() != committed.trim() {
            std::fs::write(&committed_path, &generated).expect("auto-update generated/client.ts");
            panic!(
                "codegen output drifted — auto-updated services/crm-service/generated/client.ts. \
                 Re-run to confirm."
            );
        }
    }

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

        println!("Complete Hook and Access Control DSL flow test passed!");
        println!("Parsed {} models", models.len());
        println!("Generated {} lines of Rust code", rust_code.lines().count());

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
