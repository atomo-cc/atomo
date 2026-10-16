use atomo::schema::{parse_typescript_schema, AccessRule};

fn source(builder: bool, options: &str) -> String {
    if builder {
        format!("import {{ model, text, allow }} from '@atomo-cc/schema';\nexport const Note = model('parser_notes', {{ fields: {{ id: text().id(), ownerId: text() }}, {options} }});")
    } else {
        format!("export interface Note {{\n id: string;\n ownerId: string;\n}}\nexport const schema = {{ models: {{ Note: {{ tableName: 'parser_notes', {options} }} }} }};")
    }
}

#[test]
fn access_property_spellings_preserve_runtime_rules() {
    for builder in [true, false] {
        let rule = if builder {
            "allow.role('admin')"
        } else {
            "'admin'"
        };
        for key in ["access", "'access'", "\"access\"", "'\\u0061ccess'"] {
            for space in [
                " ",
                "\n\t",
                " /* separator */ ",
                " // separator\r",
                " // separator\u{2028}",
            ] {
                let declaration = format!("{key}{space}:{space}{{ 'read'{space}:{space}{rule} }}");
                let schema = parse_typescript_schema(&source(builder, &declaration)).unwrap();
                assert!(
                    matches!(&schema.models["Note"].access.as_ref().expect("access lost").read, Some(AccessRule::Boolean(role)) if role == "admin")
                );
            }
        }
    }
}

#[test]
fn ambiguous_or_dynamic_access_rejects_runtime_loading() {
    for builder in [true, false] {
        let public = if builder {
            "allow.public()"
        } else {
            "'public'"
        };
        let admin = if builder {
            "allow.role('admin')"
        } else {
            "'admin'"
        };
        for declaration in [
            "access : externalPolicy".into(),
            "'access'\n: externalPolicy".into(),
            "...externalPolicy".into(),
            "['access']: {}".into(),
            "access".into(),
            "get access() { return {}; }".into(),
            "access: {} || externalPolicy".into(),
            "access: { ...externalPolicy }".into(),
            "access: { ['read']: 'admin' }".into(),
            format!("access: {{read:{public}}}, 'access' : {{read:{admin}}}"),
            format!("\"access\" : {{read:{admin}}}, access: {{read:{public}}}"),
            format!("access: {{read:{public}, 'read':{admin}}}"),
        ] {
            assert!(
                parse_typescript_schema(&source(builder, &declaration)).is_err(),
                "accepted builder={builder}: {declaration}"
            );
        }
    }
}

#[test]
fn duplicate_access_blocks_reject_both_orders() {
    for builder in [true, false] {
        let public = if builder {
            "allow.public()"
        } else {
            "'public'"
        };
        let admin = if builder {
            "allow.role('admin')"
        } else {
            "'admin'"
        };
        for (first, second) in [(public, admin), (admin, public)] {
            let declaration = format!("access: {{read:{first}}}, 'access' : {{read:{second}}}");
            assert!(
                parse_typescript_schema(&source(builder, &declaration)).is_err(),
                "duplicate accepted: {declaration}"
            );
        }
    }
}

#[test]
fn access_lookalikes_in_strings_comments_and_nested_options_are_not_properties() {
    for builder in [true, false] {
        let options = r#"label: "access: { read: 'admin' }", ui: { access: { read: 'admin' } }, /* access: {read:'admin'}, */ description: `access: {read:'admin'} ${`nested, access: {read:'public'}`}`"#;
        let schema = parse_typescript_schema(&source(builder, options)).unwrap();
        assert!(schema.models["Note"].access.is_none());
    }
}

#[test]
fn legacy_security_containers_are_structural_and_unambiguous() {
    let prefix = "export interface Note {\n id: string;\n}\n";
    for body in [
        "{models: {Note: {access: externalPolicy}}}",
        "{models: {Note: {access: {read:'public'}}, Note: {access: {read:'admin'}}}}",
        "{models: {Note: {}}, models: {Note: {access: {read:'admin'}}}}",
        "{models: {...externalPolicy, Note: {}}}",
        "{...externalPolicy, models: {Note: {}}}",
        "externalSchema",
    ] {
        assert!(
            parse_typescript_schema(&format!("{prefix}export const schema = {body};")).is_err(),
            "accepted {body}"
        );
    }
    let schema = parse_typescript_schema(&format!("{prefix}export const schema = {{ 'models' : {{ 'Note' : {{ 'access' : {{ 'read' : 'admin' }} }} }} }};")).unwrap();
    assert!(schema.models["Note"].access.is_some());
}

#[tokio::test]
async fn atomo_builder_rejects_access_before_database_initialization() {
    for builder in [true, false] {
        for declaration in [
            "access : externalPolicy",
            "access:{read:'public'}, 'access':{read:'admin'}",
        ] {
            let schema = source(builder, declaration);
            let parser_error = parse_typescript_schema(&schema)
                .expect_err("must reject declaration")
                .to_string();
            let runtime_error = match atomo::Atomo::builder()
                .schema_content(schema)
                .database_url("not-a-database-url")
                .build()
                .await
            {
                Ok(_) => panic!("runtime must reject before touching the database"),
                Err(error) => error.to_string(),
            };
            assert_eq!(runtime_error, parser_error);
        }
    }
}
