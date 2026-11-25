//! Lossless SII unit/field access. Edits replace only the selected value bytes.
use std::collections::HashSet;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct Unit {
    pub kind: String,
    pub id: String,
    pub span: Range<usize>,
    pub body: Range<usize>,
}

#[derive(Debug)]
pub struct Document<'a> {
    pub text: &'a str,
    pub units: Vec<Unit>,
}

// Braces in strings, escaped quotes and comments are data, never structure.
fn skip_trivia(bytes: &[u8], pos: &mut usize) -> Result<(), String> {
    loop {
        while bytes.get(*pos).is_some_and(u8::is_ascii_whitespace) {
            *pos += 1;
        }
        if bytes.get(*pos) == Some(&b'#') || bytes.get(*pos..*pos + 2) == Some(b"//") {
            while bytes.get(*pos).is_some_and(|b| *b != b'\n') {
                *pos += 1;
            }
        } else if bytes.get(*pos..*pos + 2) == Some(b"/*") {
            *pos += 2;
            while bytes.get(*pos..*pos + 2) != Some(b"*/") {
                if *pos >= bytes.len() {
                    return Err("sii_unclosed_comment".into());
                }
                *pos += 1;
            }
            *pos += 2;
        } else {
            return Ok(());
        }
    }
}

fn token<'a>(text: &'a str, pos: &mut usize) -> Result<&'a str, String> {
    skip_trivia(text.as_bytes(), pos)?;
    let start = *pos;
    while text
        .as_bytes()
        .get(*pos)
        .is_some_and(|b| !b.is_ascii_whitespace() && !b"{}:".contains(b))
    {
        *pos += 1;
    }
    if start == *pos {
        return Err("sii_token_missing".into());
    }
    Ok(&text[start..*pos])
}

fn expect(text: &str, pos: &mut usize, byte: u8) -> Result<(), String> {
    skip_trivia(text.as_bytes(), pos)?;
    if text.as_bytes().get(*pos) != Some(&byte) {
        return Err("sii_structure_invalid".into());
    }
    *pos += 1;
    Ok(())
}

/// Also used by the existing unit parser for standalone unit fragments.
pub fn scan_units(text: &str, require_container: bool) -> Result<Vec<Unit>, String> {
    let bytes = text.as_bytes();
    let mut pos = if text.starts_with('\u{feff}') { 3 } else { 0 };
    skip_trivia(bytes, &mut pos)?;
    let container = text[pos..].starts_with("SiiNunit");
    if require_container && !container {
        return Err("sii_container_missing".into());
    }
    if container {
        if token(text, &mut pos)? != "SiiNunit" {
            return Err("sii_container_invalid".into());
        }
        expect(text, &mut pos, b'{')?;
    }
    let mut units = Vec::new();
    let mut closed = !container;
    loop {
        skip_trivia(bytes, &mut pos)?;
        if pos == bytes.len() {
            break;
        }
        if container && !closed && bytes[pos] == b'}' {
            pos += 1;
            skip_trivia(bytes, &mut pos)?;
            closed = true;
            if pos != bytes.len() {
                if require_container {
                    return Err("sii_trailing_data".into());
                }
                continue;
            }
            break;
        }
        let start = pos;
        let kind = token(text, &mut pos)?.to_string();
        if !kind.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err("sii_unit_type_invalid".into());
        }
        expect(text, &mut pos, b':')?;
        let id = token(text, &mut pos)?.to_string();
        expect(text, &mut pos, b'{')?;
        let body_start = pos;
        let mut depth = 1;
        while pos < bytes.len() && depth > 0 {
            match bytes[pos] {
                b'"' => {
                    pos += 1;
                    loop {
                        match bytes.get(pos) {
                            Some(b'\\') => {
                                pos += 2;
                            }
                            Some(b'"') => {
                                pos += 1;
                                break;
                            }
                            Some(_) => {
                                pos += 1;
                            }
                            None => return Err("sii_unclosed_string".into()),
                        }
                    }
                }
                b'#' => {
                    while pos < bytes.len() && bytes[pos] != b'\n' {
                        pos += 1;
                    }
                }
                b'/' if bytes.get(pos + 1).is_some_and(|b| *b == b'/' || *b == b'*') => {
                    skip_trivia(bytes, &mut pos)?;
                }
                b'{' => {
                    depth += 1;
                    pos += 1;
                }
                b'}' => {
                    depth -= 1;
                    pos += 1;
                }
                _ => {
                    pos += 1;
                }
            }
        }
        if depth != 0 {
            return Err("sii_unclosed_unit".into());
        }
        units.push(Unit {
            kind,
            id,
            span: start..pos,
            body: body_start..pos - 1,
        });
    }
    if !closed {
        return Err("sii_unclosed_container".into());
    }
    Ok(units)
}

impl<'a> Document<'a> {
    pub fn parse(text: &'a str) -> Result<Self, String> {
        let units = scan_units(text, true)?;
        let mut ids = HashSet::new();
        if units.iter().any(|u| !ids.insert(u.id.to_ascii_lowercase())) {
            return Err("sii_duplicate_unit".into());
        }
        Ok(Self { text, units })
    }

    pub fn unique(&self, kind: &str) -> Result<&Unit, String> {
        let mut found = self.units.iter().filter(|u| u.kind == kind);
        let unit = found
            .next()
            .ok_or_else(|| format!("sii_unit_missing:{kind}"))?;
        if found.next().is_some() {
            return Err(format!("sii_unit_ambiguous:{kind}"));
        }
        Ok(unit)
    }

    pub fn resolve(&self, id: &str, kind: &str) -> Result<&Unit, String> {
        self.units
            .iter()
            .find(|u| u.id.eq_ignore_ascii_case(id) && u.kind == kind)
            .ok_or_else(|| format!("sii_reference_missing:{kind}"))
    }

    pub fn field_span(&self, unit: &Unit, key: &str) -> Result<Range<usize>, String> {
        let mut found = None;
        let mut pos = unit.body.start;
        // Only inspect top-level field lines; quoted/multiline unknown data stays opaque.
        let mut depth = 0i32;
        let mut quoted = false;
        let mut block_comment = false;
        while pos < unit.body.end {
            let end = self.text[pos..unit.body.end]
                .find('\n')
                .map(|n| pos + n + 1)
                .unwrap_or(unit.body.end);
            let line = &self.text[pos..end];
            if depth == 0 && !quoted && !block_comment {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix(key).and_then(|s| s.strip_prefix(':')) {
                    let leading = line.len() - trimmed.len() + key.len() + 1;
                    let value_start = pos + leading + rest.len() - rest.trim_start().len();
                    let mut value_end = end;
                    let mut in_string = false;
                    let mut escaped = false;
                    for (i, b) in self.text.as_bytes()[value_start..end].iter().enumerate() {
                        if escaped {
                            escaped = false;
                            continue;
                        }
                        if in_string && *b == b'\\' {
                            escaped = true;
                            continue;
                        }
                        if *b == b'"' {
                            in_string = !in_string;
                        }
                        if !in_string
                            && (*b == b'#'
                                || (*b == b'/'
                                    && matches!(
                                        self.text.as_bytes().get(value_start + i + 1),
                                        Some(b'/') | Some(b'*')
                                    )))
                        {
                            value_end = value_start + i;
                            break;
                        }
                    }
                    value_end = value_start + self.text[value_start..value_end].trim_end().len();
                    if value_end == value_start || found.replace(value_start..value_end).is_some() {
                        return Err(format!("sii_field_invalid:{key}"));
                    }
                }
            }
            let bytes = line.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if block_comment {
                    if bytes.get(i..i + 2) == Some(b"*/") {
                        block_comment = false;
                        i += 2;
                    } else {
                        i += 1;
                    }
                } else if quoted {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else {
                        if bytes[i] == b'"' {
                            quoted = false;
                        }
                        i += 1;
                    }
                } else if bytes[i] == b'#' || bytes.get(i..i + 2) == Some(b"//") {
                    break;
                } else if bytes.get(i..i + 2) == Some(b"/*") {
                    block_comment = true;
                    i += 2;
                } else {
                    match bytes[i] {
                        b'"' => quoted = true,
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        _ => {}
                    }
                    i += 1;
                }
            }
            pos = end;
        }
        found.ok_or_else(|| format!("sii_field_missing:{key}"))
    }

    pub fn field(&self, unit: &Unit, key: &str) -> Result<&'a str, String> {
        Ok(&self.text[self.field_span(unit, key)?])
    }

    pub fn patch(&self, unit: &Unit, key: &str, value: &str) -> Result<String, String> {
        if value.contains(['\n', '\r', '{', '}']) {
            return Err("sii_value_invalid".into());
        }
        let span = self.field_span(unit, key)?;
        let mut text = self.text.to_string();
        text.replace_range(span, value);
        Document::parse(&text)?;
        Ok(text)
    }

    pub fn bank(&self) -> Result<&Unit, String> {
        let economy = self.unique("economy")?;
        self.resolve(self.field(economy, "bank")?, "bank")
    }

    pub fn player_driver(&self) -> Result<&Unit, String> {
        let economy = self.unique("economy")?;
        let player = self.resolve(self.field(economy, "player")?, "player")?;
        match self.field(player, "driver") {
            Ok(id) => self.resolve(id, "driver_player"),
            Err(error) if error == "sii_field_missing:driver" => self.unique("driver_player"),
            Err(error) => Err(error),
        }
    }
}

pub fn patch_money(text: &str, value: i64) -> Result<String, String> {
    if !(0..=9_007_199_254_740_991).contains(&value) {
        return Err("money_invalid".into());
    }
    let doc = Document::parse(text)?;
    let bank = doc.bank()?;
    doc.field(bank, "money_account")?
        .parse::<i64>()
        .map_err(|_| "money_invalid")?;
    doc.patch(bank, "money_account", &value.to_string())
}

pub fn patch_skill(text: &str, key: &str, value: i64) -> Result<String, String> {
    let max = match key {
        "adr" => 63,
        "long_dist" | "heavy" | "fragile" | "urgent" | "mechanical" => 6,
        _ => return Err("skill_invalid".into()),
    };
    if !(0..=max).contains(&value) {
        return Err("skill_invalid".into());
    }
    let doc = Document::parse(text)?;
    doc.patch(doc.unique("economy")?, key, &value.to_string())
}
