use atomo::schema::{parse_typescript_schema, AccessRule};
use atomo_schema::DslParser;

fn source(declaration: &str) -> String {
    format!("export interface Note {{\n id:string;\n ownerId:string;\n}}\n{declaration}\nexport const schema = {{models: {{Note: {{}}}}}};")
}

fn rejects(options: &str) {
    let text = source(&format!(
        "export const NoteModel = defineModel({{{options}}});"
    ));
    assert!(
        parse_typescript_schema(&text).is_err(),
        "runtime accepted {options}"
    );
    assert!(DslParser::new().parse_define_model(&text).is_err());
    assert!(DslParser::new()
        .parse_model_definition(&text, "Note")
        .is_err());
}

#[test]
fn compatibility_variable_access_is_rejected() {
    rejects("access : externalPolicy");
}

#[test]
fn compatibility_quoted_callback_is_rejected() {
    rejects("'access': { read: ({ user }: access.Context<User>) => user.role === 'admin' }");
}

#[test]
fn compatibility_spaced_callback_is_rejected() {
    rejects("access: { read : ({ user }: access.Context<User>) => user.role === 'admin' }");
}

#[test]
fn compatibility_aliases_preserve_declarative_rules() {
    for (prefix, call) in [
        ("", "defineModel"),
        (
            "import {defineModel as secureModel} from '@atomo-cc/schema';",
            "secureModel",
        ),
        ("const secureModel = defineModel;", "secureModel"),
    ] {
        let text = source(&format!("{prefix}\nexport const NoteModel = {call} /* call */ ({{'access' /* key */ : {{'read' : 'admin', update: allow.owner('ownerId')}}}});"));
        let schema = parse_typescript_schema(&text).unwrap();
        let access = schema.models["Note"].access.as_ref().expect("rules lost");
        assert!(matches!(&access.read, Some(AccessRule::Boolean(role)) if role == "admin"));
        assert!(matches!(&access.update, Some(AccessRule::Owner(field)) if field == "ownerId"));
    }
}

#[test]
fn compatibility_ambiguity_and_unresolvable_calls_fail_closed() {
    for declaration in [
        "export const NoteModel = defineModel({access:{read:'public'}, 'access':{read:'admin'}});",
        "export const NoteModel = defineModel({access:{read:'public', 'read':'admin'}});",
        "export const NoteModel = defineModel({...externalPolicy});",
        "export const NoteModel = defineModel({['access']: externalPolicy});",
        "export const NoteModel = defineModel(externalPolicy);",
        "export const NoteModel = defineModel({access:{read:'public'}}); export const NoteModel = defineModel({access:{read:'admin'}});",
        "export const NotePolicy = defineModel({access:{read:'admin'}});",
        "const {defineModel: secureModel} = external; export const NoteModel = secureModel({access:externalPolicy});",
        "const secureModel = external.defineModel; export const NoteModel = secureModel({access:externalPolicy});",
        "const secureModel = external['defineModel']; export const NoteModel = secureModel({access:externalPolicy});",
        "const secureModel = defineModel.bind(external); export const NoteModel = secureModel({access:externalPolicy});",
        "import secureModel from '@atomo-cc/schema'; export const NoteModel = secureModel({access:externalPolicy});",
        "import secureModel from '@atomo-cc/schema'; export const Note = secureModel({access:externalPolicy});",
        "export const NoteModel = defineModel({'access': {'read': () => true}});",
    ] {
        assert!(parse_typescript_schema(&source(declaration)).is_err(), "accepted {declaration}");
    }
}

#[test]
fn compatibility_lookalikes_and_shared_entry_points() {
    let text = source(
        r#"
// export const NoteModel = defineModel({access : externalPolicy});
const description = "export const NoteModel = defineModel({access : externalPolicy});";
const ui = {defineModel: 'data'};
export const NoteModel = defineModel({description: `defineModel: ${'data'}`, ui: {access: 'data'}, 'access': {read:'admin'}});
"#,
    );
    let schema = parse_typescript_schema(&text).unwrap();
    assert!(schema.models["Note"].access.is_some());
    let parsed = atomo_schema::TypeScriptParser::new().parse(&text).unwrap();
    assert!(parsed
        .iter()
        .find(|model| model.name == "Note")
        .unwrap()
        .access
        .is_some());
    let mixed = format!("import {{model,text}} from '@atomo-cc/schema';\nexport const Other = model('other', {{fields: {{id: text().id()}}}});\n{text}");
    assert!(parse_typescript_schema(&mixed).is_err());
    assert!(atomo_schema::parse_builder_dsl(&text).is_err());
}

#[tokio::test]
async fn compatibility_runtime_rejects_before_database_initialization() {
    let text = source("export const NoteModel = defineModel({access : externalPolicy});");
    let parser_error = parse_typescript_schema(&text)
        .expect_err("must reject")
        .to_string();
    let runtime_error = match atomo::Atomo::builder()
        .schema_content(text)
        .database_url("not-a-database-url")
        .build()
        .await
    {
        Ok(_) => panic!("runtime accepted unsupported access"),
        Err(error) => error.to_string(),
    };
    assert_eq!(parser_error, runtime_error);
}
