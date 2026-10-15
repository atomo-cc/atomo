//! Structural subset of TypeScript object syntax used for security declarations.
//! No evaluation: dynamic keys, spreads, methods and shorthand are rejected.
use anyhow::{bail, ensure, Result};
use std::collections::HashSet;

#[derive(Debug)]
enum Kind {
    Word,
    String(String),
    Punct(char),
}

struct Token<'a> {
    raw: &'a str,
    kind: Kind,
    start: usize,
    end: usize,
}

fn string_value(raw: &str) -> Result<String> {
    let mut chars = raw[1..raw.len() - 1].chars();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let escape = chars
            .next()
            .ok_or_else(|| anyhow::anyhow!("Incomplete string escape"))?;
        match escape {
            'u' | 'x' => {
                let mut hex = String::new();
                if escape == 'u' && chars.clone().next() == Some('{') {
                    chars.next();
                    loop {
                        let ch = chars
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("Incomplete Unicode escape"))?;
                        if ch == '}' {
                            break;
                        }
                        hex.push(ch);
                    }
                } else {
                    for _ in 0..if escape == 'u' { 4 } else { 2 } {
                        hex.push(
                            chars
                                .next()
                                .ok_or_else(|| anyhow::anyhow!("Incomplete Unicode escape"))?,
                        );
                    }
                }
                out.push(
                    char::from_u32(u32::from_str_radix(&hex, 16)?)
                        .ok_or_else(|| anyhow::anyhow!("Unsupported Unicode escape"))?,
                );
            }
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '\n' => (),
            '\r' => {
                if chars.clone().next() == Some('\n') {
                    chars.next();
                }
            }
            other => out.push(other),
        }
    }
    Ok(out)
}

fn literal_end(source: &str, start: usize) -> Result<usize> {
    let quote = source.as_bytes()[start] as char;
    let mut i = start + 1;
    while i < source.len() {
        let ch = source[i..].chars().next().unwrap();
        ensure!(
            quote == '`' || !matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}'),
            "Unescaped newline in string"
        );
        i += ch.len_utf8();
        if ch == quote {
            return Ok(i);
        }
        if ch == '\\' {
            ensure!(i < source.len(), "Unterminated string escape");
            let escaped = source[i..].chars().next().unwrap();
            i += escaped.len_utf8();
            if escaped == '\r' && source[i..].starts_with('\n') {
                i += 1;
            }
        } else if quote == '`' && ch == '$' && source[i..].starts_with('{') {
            i = interpolation_end(source, i + 1)?;
        }
    }
    bail!("Unterminated string")
}

fn interpolation_end(source: &str, mut i: usize) -> Result<usize> {
    let mut depth = 1;
    while i < source.len() {
        if source[i..].starts_with("//") {
            i += source[i..]
                .find(['\n', '\r', '\u{2028}', '\u{2029}'])
                .unwrap_or(source.len() - i);
            continue;
        }
        if source[i..].starts_with("/*") {
            i += source[i + 2..]
                .find("*/")
                .ok_or_else(|| anyhow::anyhow!("Unterminated comment"))?
                + 4;
            continue;
        }
        let ch = source[i..].chars().next().unwrap();
        if matches!(ch, '\'' | '"' | '`') {
            i = literal_end(source, i)?;
            continue;
        }
        i += ch.len_utf8();
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(i);
                }
            }
            // Regex literals require a full JS expression parser. Reject rather
            // than let a brace/backtick inside one alter template boundaries.
            '/' => bail!("Unsupported slash expression in template interpolation"),
            _ => (),
        }
    }
    bail!("Unterminated template interpolation")
}

fn tokens(source: &str) -> Result<Vec<Token<'_>>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < source.len() {
        let c = source[i..].chars().next().unwrap();
        if c.is_whitespace() || c == '\u{feff}' {
            i += c.len_utf8();
            continue;
        }
        if source[i..].starts_with("//") {
            i += source[i..]
                .find(['\n', '\r', '\u{2028}', '\u{2029}'])
                .unwrap_or(source.len() - i);
            continue;
        }
        if source[i..].starts_with("/*") {
            i += source[i + 2..]
                .find("*/")
                .ok_or_else(|| anyhow::anyhow!("Unterminated comment"))?
                + 4;
            continue;
        }
        let start = i;
        ensure!(c != '\\', "Unsupported escaped identifier");
        let kind = if matches!(c, '\'' | '"' | '`') {
            i = literal_end(source, start)?;
            if c == '`' {
                Kind::Punct('`')
            } else {
                Kind::String(string_value(&source[start..i])?)
            }
        } else if c.is_alphanumeric() || matches!(c, '_' | '$') {
            i += c.len_utf8();
            while i < source.len() {
                let ch = source[i..].chars().next().unwrap();
                if !ch.is_alphanumeric() && !matches!(ch, '_' | '$') {
                    break;
                }
                i += ch.len_utf8();
            }
            Kind::Word
        } else {
            i += c.len_utf8();
            Kind::Punct(c)
        };
        out.push(Token {
            raw: &source[start..i],
            kind,
            start,
            end: i,
        });
    }
    Ok(out)
}

fn end_group(ts: &[Token<'_>], start: usize) -> Result<usize> {
    let mut stack = Vec::new();
    for (i, t) in ts.iter().enumerate().skip(start) {
        match t.kind {
            Kind::Punct('(') => stack.push(')'),
            Kind::Punct('[') => stack.push(']'),
            Kind::Punct('{') => stack.push('}'),
            Kind::Punct(c @ (')' | ']' | '}')) => {
                ensure!(stack.pop() == Some(c), "Unbalanced declaration");
                if stack.is_empty() {
                    return Ok(i);
                }
            }
            _ => (),
        }
    }
    bail!("Unterminated declaration")
}

/// Parse only direct properties in an object body, preserving complete values.
pub(crate) fn properties(body: &str) -> Result<Vec<(String, String)>> {
    let ts = tokens(body)?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut i = 0;
    while i < ts.len() {
        let key = match &ts[i].kind {
            Kind::Word => ts[i].raw.to_owned(),
            Kind::String(s) => s.clone(),
            _ => bail!("Unsupported object property (spread or computed key)"),
        };
        ensure!(seen.insert(key.clone()), "Duplicate object property: {key}");
        i += 1;
        ensure!(
            ts.get(i).is_some_and(|t| t.raw == ":"),
            "Unsupported object property: {key}"
        );
        i += 1;
        let start = i;
        while i < ts.len() && ts[i].raw != "," {
            if matches!(ts[i].kind, Kind::Punct('(' | '[' | '{')) {
                i = end_group(&ts, i)?;
            } else {
                ensure!(
                    !matches!(ts[i].kind, Kind::Punct(')' | ']' | '}')),
                    "Unbalanced value"
                );
            }
            i += 1;
        }
        ensure!(i > start, "Missing value for {key}");
        // Whitespace separates tokens after comments are removed; quoted contents
        // are kept intact, so strings containing commas/braces remain data.
        let value = ts[start..i]
            .iter()
            .map(|t| t.raw)
            .collect::<Vec<_>>()
            .join(" ");
        out.push((key, value));
        if i < ts.len() {
            i += 1;
        }
    }
    Ok(out)
}

pub(crate) fn object_body(value: &str) -> Result<&str> {
    let ts = tokens(value)?;
    ensure!(
        ts.first().is_some_and(|t| t.raw == "{"),
        "Expected inline object declaration"
    );
    let end = end_group(&ts, 0)?;
    ensure!(end + 1 == ts.len(), "Unsupported object expression suffix");
    Ok(&value[ts[0].end..ts[end].start])
}

pub(crate) fn legacy_models(source: &str) -> Result<Vec<(String, String)>> {
    let ts = tokens(source)?;
    let mut result = None;
    for i in 0..ts.len().saturating_sub(3) {
        if ts[i..i + 4]
            .iter()
            .map(|t| t.raw)
            .eq(["export", "const", "schema", "="])
        {
            ensure!(result.is_none(), "Duplicate schema declaration");
            ensure!(
                ts.get(i + 4).is_some_and(|t| t.raw == "{"),
                "Expected inline schema declaration"
            );
            let end = end_group(&ts, i + 4)?;
            ensure!(
                ts.get(end + 1)
                    .is_none_or(|t| matches!(t.raw, ";" | "export")),
                "Unsupported schema expression"
            );
            let props = properties(&source[ts[i + 4].end..ts[end].start])?;
            let mut models = Vec::new();
            if let Some((_, value)) = props.iter().find(|(k, _)| k == "models") {
                for (name, definition) in properties(object_body(value)?)? {
                    models.push((name, object_body(&definition)?.to_owned()));
                }
            }
            result = Some(models);
        }
    }
    Ok(result.unwrap_or_default())
}

pub(crate) fn model_blocks(source: &str) -> Result<Vec<(String, String, String)>> {
    let ts = tokens(source)?;
    let mut out = Vec::new();
    let mut names = HashSet::new();
    for i in 0..ts.len().saturating_sub(4) {
        if ts[i].raw != "export"
            || ts[i + 1].raw != "const"
            || ts[i + 3].raw != "="
            || ts[i + 4].raw != "model"
        {
            continue;
        }
        ensure!(ts.len() > i + 8, "Incomplete model declaration");
        ensure!(ts[i + 5].raw == "(", "Unsupported model declaration");
        let Kind::String(table) = &ts[i + 6].kind else {
            bail!("Expected literal model table");
        };
        ensure!(
            ts[i + 7].raw == "," && ts[i + 8].raw == "{",
            "Expected inline model options"
        );
        let end = end_group(&ts, i + 8)?;
        ensure!(
            ts.get(end + 1).is_some_and(|t| t.raw == ")"),
            "Unsupported model options expression"
        );
        ensure!(names.insert(ts[i + 2].raw), "Duplicate model declaration");
        out.push((
            ts[i + 2].raw.to_owned(),
            table.clone(),
            source[ts[i + 8].end..ts[end].start].to_owned(),
        ));
    }
    Ok(out)
}

/// Collect every compatibility factory invocation, including named-import and
/// immutable local aliases. Unknown reference shapes are errors, never ignored.
pub(crate) fn compatibility_blocks(source: &str) -> Result<Vec<(String, String)>> {
    let ts = tokens(source)?;
    let interface_names: HashSet<_> = ts
        .windows(3)
        .filter(|window| window[0].raw == "export" && window[1].raw == "interface")
        .map(|window| window[2].raw)
        .collect();
    let mut aliases: HashSet<String> = ["defineModel".to_owned()].into();
    let mut imports = HashSet::new();
    for i in 0..ts.len().saturating_sub(1) {
        if ts[i].raw != "import" || ts[i + 1].raw != "{" {
            continue;
        }
        let end = end_group(&ts, i + 1)?;
        for j in i + 2..end {
            imports.insert(j);
            if ts[j].raw == "defineModel" && ts.get(j + 1).is_some_and(|t| t.raw == "as") {
                let alias = ts
                    .get(j + 2)
                    .ok_or_else(|| anyhow::anyhow!("Missing defineModel import alias"))?;
                ensure!(
                    matches!(alias.kind, Kind::Word),
                    "Unsupported defineModel import alias"
                );
                aliases.insert(alias.raw.to_owned());
            }
        }
    }
    let mut alias_refs = HashSet::new();
    loop {
        let mut changed = false;
        for i in 0..ts.len().saturating_sub(3) {
            if ts[i].raw == "const"
                && matches!(ts[i + 1].kind, Kind::Word)
                && ts[i + 2].raw == "="
                && aliases.contains(ts[i + 3].raw)
                && !ts.get(i + 4).is_some_and(|t| t.raw == "(")
            {
                ensure!(
                    ts.get(i + 4)
                        .is_none_or(|t| matches!(t.raw, ";" | "export" | "const")),
                    "Unsupported defineModel alias expression"
                );
                alias_refs.insert(i + 3);
                alias_refs.insert(i + 1);
                changed |= aliases.insert(ts[i + 1].raw.to_owned());
            }
        }
        if !changed {
            break;
        }
    }
    let mut out = Vec::new();
    let mut names = HashSet::new();
    for i in 0..ts.len().saturating_sub(1) {
        if ts[i].raw == "const"
            && ts
                .get(i + 1)
                .is_some_and(|t| t.raw.ends_with("Model") || interface_names.contains(t.raw))
            && ts.get(i + 2).is_some_and(|t| t.raw == "=")
            && !alias_refs.contains(&(i + 1))
        {
            ensure!(
                ts.get(i + 3)
                    .is_some_and(|t| aliases.contains(t.raw) || t.raw == "model"),
                "Unsupported compatibility model factory or alias"
            );
        }
        if matches!(ts[i].raw, "const" | "let" | "var") && ts[i + 1].raw == "{" {
            let end = end_group(&ts, i + 1)?;
            ensure!(
                !ts[i + 2..end].iter().any(|t| aliases.contains(t.raw)),
                "Unsupported defineModel destructuring alias"
            );
        }
    }
    for (i, token) in ts.iter().enumerate() {
        if let Kind::String(name) = &token.kind {
            if aliases.contains(name)
                && i >= 2
                && ts[i - 1].raw == "["
                && matches!(ts[i - 2].kind, Kind::Word)
            {
                bail!("Unsupported computed defineModel reference");
            }
        }
        if !matches!(token.kind, Kind::Word)
            || !aliases.contains(token.raw)
            || imports.contains(&i)
            || alias_refs.contains(&i)
        {
            continue;
        }
        if ts.get(i + 1).is_some_and(|t| t.raw == ":") {
            continue;
        }
        ensure!(
            ts.get(i + 1).is_some_and(|t| t.raw == "("),
            "Unsupported defineModel reference"
        );
        ensure!(
            i >= 3
                && ts[i - 3].raw == "const"
                && ts[i - 1].raw == "="
                && matches!(ts[i - 2].kind, Kind::Word),
            "defineModel requires a named const declaration"
        );
        let name = ts[i - 2].raw;
        let model = name.strip_suffix("Model").unwrap_or(name);
        ensure!(
            !model.is_empty() && names.insert(model.to_owned()),
            "Duplicate defineModel declaration for {model}"
        );
        let end = end_group(&ts, i + 1)?;
        let args = &source[ts[i + 1].end..ts[end].start];
        let body = object_body(args)?;
        ensure!(
            ts.get(end + 1)
                .is_none_or(|t| matches!(t.raw, ";" | "export" | "const")),
            "Unsupported defineModel expression suffix"
        );
        properties(body)?;
        out.push((model.to_owned(), body.to_owned()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries_ignore_nested_data_and_comments() {
        let props = properties(r#"note: 'access:{read:"public"}', /* }, */ ui: {access: {}}, 'access' /* : */ : {"read": 'admin'}"#).unwrap();
        assert_eq!(props.len(), 3);
        assert_eq!(props[2].0, "access");
        assert_eq!(
            properties(object_body(&props[2].1).unwrap()).unwrap()[0].0,
            "read"
        );
    }

    #[test]
    fn ambiguity_and_malformed_structure_fail_closed() {
        for body in [
            "access: {}, '\\u0061ccess': {}",
            "...policy",
            "[name]: {}",
            "access",
            "access: {",
            "access: /*",
        ] {
            assert!(properties(body).is_err(), "{body}");
        }
        assert!(object_body("{} || policy").is_err());
    }
}
