//! Úkoly `[ ] text`, `[x] text`, `[!] text`, včetně atributů `@due:2026-08-15`.
//!
//! Task management není součástí MVP, ale syntaxe se parsuje už teď, aby ji
//! šlo později využít beze změny formátu.

use crate::text::{char_before, Line, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// `[ ]`
    Open,
    /// `[x]` / `[X]`
    Done,
    /// `[!]`
    Important,
    /// Jakýkoli jiný jednoznakový stav — formát zůstává otevřený.
    Other(char),
}

impl TaskState {
    fn from_char(ch: char) -> Self {
        match ch {
            ' ' => TaskState::Open,
            'x' | 'X' => TaskState::Done,
            '!' => TaskState::Important,
            other => TaskState::Other(other),
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(self, TaskState::Done)
    }
}

/// Inline atribut úkolu, např. `@due:2026-08-15`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskAttribute {
    pub key: String,
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub state: TaskState,
    /// Text úkolu za `[ ]`, včetně atributů, ořezaný o okolní whitespace.
    pub text: String,
    pub text_span: Span,
    pub attributes: Vec<TaskAttribute>,
    /// Celý řádek úkolu.
    pub span: Span,
    /// Rozsah samotného `[ ]` — pro překlikávání stavu v UI.
    pub marker_span: Span,
    pub line: usize,
    /// Odsazení v bytech, pro budoucí vnořené úkoly.
    pub indent: usize,
}

fn is_key_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-')
}

/// Najde atributy `@klic:hodnota` uvnitř textu úkolu.
fn parse_attributes(text: &str, base: usize) -> Vec<TaskAttribute> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    while let Some(rel) = text[cursor..].find('@') {
        let at = cursor + rel;
        cursor = at + 1;

        if let Some(prev) = char_before(text, at) {
            if !prev.is_whitespace() {
                continue;
            }
        }

        let rest = &text[at + 1..];
        let key_len = rest
            .char_indices()
            .find(|(_, ch)| !is_key_char(*ch))
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        if key_len == 0 || !rest[key_len..].starts_with(':') {
            continue;
        }

        let value_start = key_len + 1;
        let value_len = rest[value_start..]
            .char_indices()
            .find(|(_, ch)| ch.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(rest.len() - value_start);
        if value_len == 0 {
            continue;
        }

        out.push(TaskAttribute {
            key: rest[..key_len].to_string(),
            value: rest[value_start..value_start + value_len].to_string(),
            span: Span::new(base + at, base + at + 1 + value_start + value_len),
        });
        cursor = at + 1 + value_start + value_len;
    }

    out
}

fn parse_line(line: &Line<'_>) -> Option<Task> {
    let text = line.text;
    let indent = text.len() - text.trim_start().len();
    let mut rel = indent;

    // Úkol může být zapsaný i jako položka seznamu: `- [ ] text`.
    if text[rel..].starts_with("- ") {
        rel += 2;
        rel += text[rel..].len() - text[rel..].trim_start().len();
    }

    let rest = &text[rel..];
    if !rest.starts_with('[') {
        return None;
    }
    let mut chars = rest[1..].chars();
    let state_ch = chars.next()?;
    if state_ch == ']' {
        return None;
    }
    let marker_len = 1 + state_ch.len_utf8() + 1;
    if !rest[1 + state_ch.len_utf8()..].starts_with(']') {
        return None;
    }
    // Za `]` musí být konec řádku nebo mezera — `[x]y` úkol není.
    let after = &rest[marker_len..];
    if !(after.is_empty() || after.starts_with(char::is_whitespace)) {
        return None;
    }

    let body_rel = rel + marker_len;
    let body_raw = &text[body_rel..];
    let lead = body_raw.len() - body_raw.trim_start().len();
    let body = body_raw.trim();
    let body_start = line.abs(body_rel + lead);

    Some(Task {
        state: TaskState::from_char(state_ch),
        text: body.to_string(),
        text_span: Span::new(body_start, body_start + body.len()),
        attributes: parse_attributes(body, body_start),
        span: line.span(),
        marker_span: Span::new(line.abs(rel), line.abs(rel + marker_len)),
        line: line.number,
        indent,
    })
}

pub fn parse(lines: &[Line<'_>]) -> Vec<Task> {
    lines.iter().filter_map(parse_line).collect()
}
