use atomo_schema::{parse_builder_dsl, ActionInputDef, ActionReturn};

const SCHEMA: &str = r#"
import { model, text, action, allow } from '@atomo-cc/schema';
export const Parent = model('parents', {
  fields: { id: text().id(), ownerId: text().fromAuth('userId') },
  access: {
    read: allow.owner('ownerId'),
    update: allow.all([allow.authenticated(), allow.any([allow.owner('ownerId'), allow.role('admin')])]),
  },
});
export const inspect = action('inspect').input({ recordId: text().required(), note: text().optional() }).returns('parents');
"#;

#[test]
fn m1_owner_and_composition_are_preserved() {
    let schema = parse_builder_dsl(SCHEMA).unwrap();
    let access = schema.models["Parent"].access.as_ref().unwrap();
    assert!(serde_json::to_value(&access.read)
        .unwrap()
        .get("Owner")
        .is_some());
    assert!(serde_json::to_value(&access.update)
        .unwrap()
        .get("And")
        .is_some());
}

#[test]
fn m1_sources_and_action_contract_survive_parsing() {
    let schema = parse_builder_dsl(SCHEMA).unwrap();
    assert!(schema.models["Parent"].fields["ownerId"]
        .attributes
        .iter()
        .any(|a| serde_json::to_value(a).unwrap().get("FromAuth").is_some()));
    let action = &schema.actions["inspect"];
    assert!(matches!(&action.returns, Some(ActionReturn::Model(m)) if m == "Parent"));
    let ActionInputDef::Object { fields } = &action.input else {
        panic!("object input lost")
    };
    assert_eq!(fields.len(), 2);
    assert!(fields[0].required);
    assert!(!fields[1].required);
}

#[test]
fn m1_unknown_security_declarations_fail_closed() {
    for bad in [
        SCHEMA.replace("allow.owner('ownerId')", "allow.customRule('ownerId')"),
        SCHEMA.replace("fromAuth('userId')", "fromAuth('untrusted')"),
        SCHEMA.replace("allow.owner('ownerId')", "allow.owner('missing')"),
        SCHEMA.replace(
            ".returns('parents')",
            ".access(allow.role('admin')).returns('parents')",
        ),
        SCHEMA.replace(".returns('parents')", ".returns('missing')"),
    ] {
        assert!(parse_builder_dsl(&bad).is_err());
    }
}

#[test]
fn m1_builder_and_legacy_pick_actions_are_equivalent() {
    let builder = parse_builder_dsl(r#"
      export const Parent = model('parents', { fields: { id: text().id(), title: text().required() } });
      export const inspect = action('inspect').from('parents').input(['id', 'title']).returns('parents');
    "#).unwrap();
    let legacy = atomo_schema::TypeScriptParser::new().parse_schema(r#"
      export interface Parent { id: string; title: string; }
      export const schema = { models: { Parent: { tableName: 'parents' } },
        actions: { inspect: { input: { pick: { model: 'Parent', fields: ['id', 'title'] } }, returns: 'Parent' } } };
    "#).unwrap();
    assert_eq!(
        serde_json::to_value(&builder.actions["inspect"]).unwrap(),
        serde_json::to_value(&legacy.actions["inspect"]).unwrap()
    );
}

#[test]
fn m1_builder_and_legacy_object_actions_are_equivalent() {
    let builder = parse_builder_dsl(SCHEMA).unwrap();
    let legacy = atomo_schema::TypeScriptParser::new().parse_schema(r#"
      export interface Parent { id: string; }
      export const schema = { models: { Parent: { tableName: 'parents' } }, actions: {
        inspect: { input: { object: { recordId: {type:'string', required:true}, note: {type:'string', required:false} } }, returns:'Parent' }
      } };
    "#).unwrap();
    assert_eq!(
        serde_json::to_value(&builder.actions["inspect"]).unwrap(),
        serde_json::to_value(&legacy.actions["inspect"]).unwrap()
    );
}
