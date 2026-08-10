//! Sekce `== Nadpis ==`, volitelně se stabilním ID `== Nadpis == ^id`.
//!
//! Nadpisem je pouze řádek, na kterém kromě `==`, textu nadpisu a případného
//! `^id` není nic dalšího. Tím se `== Instalace ==` odliší od běžné věty, ve
//! které se `==` mihne.

use crate::text::{Line, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Nadpis v původním zápisu (dohledávání je case-insensitive).
    pub title: String,
    pub title_span: Span,
    /// Stabilní ID bez `^`, pokud je uvedeno.
    pub block_id: Option<String>,
    /// Řádek s nadpisem.
    pub span: Span,
    /// Obsah sekce: od konce řádku s nadpisem po začátek dalšího nadpisu
    /// (nebo konec souboru).
    pub content_span: Span,
    pub line: usize,
    /// Úroveň zanoření podle počtu `=` (`==` = 1, `===` = 2, …).
    pub level: usize,
}

impl Section {
    /// Porovnání nadpisu pro potřeby odkazu `[[Poznámka#Sekce]]`.
    pub fn matches(&self, name: &str) -> bool {
        self.title.to_lowercase() == name.trim().to_lowercase()
    }
}

fn is_id_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-')
}

struct Heading {
    title: String,
    title_rel: usize,
    block_id: Option<String>,
    level: usize,
}

fn parse_heading(text: &str) -> Option<Heading> {
    let lead = text.len() - text.trim_start().len();
    let trimmed = text.trim();
    if !trimmed.starts_with("==") {
        return None;
    }

    // Volitelné `^id` na konci řádku.
    let (before_id, block_id) = match trimmed.rsplit_once('^') {
        Some((head, id))
            if !id.is_empty()
                && id.chars().all(is_id_char)
                && head.trim_end().ends_with("==")
                && head.ends_with(' ') =>
        {
            (head.trim_end(), Some(id.to_string()))
        }
        _ => (trimmed, None),
    };

    let level = before_id.chars().take_while(|c| *c == '=').count();
    if level < 2 {
        return None;
    }
    let open_len = level;
    let rest = &before_id[open_len..];
    let close_len = rest.chars().rev().take_while(|c| *c == '=').count();
    if close_len != level || rest.len() <= close_len {
        return None;
    }

    let inner = &rest[..rest.len() - close_len];
    let title = inner.trim();
    if title.is_empty() || title.contains('=') {
        return None;
    }

    let inner_lead = inner.len() - inner.trim_start().len();
    Some(Heading {
        title: title.to_string(),
        title_rel: lead + open_len + inner_lead,
        block_id,
        level,
    })
}

/// Najde sekce v poznámce. `end` je offset konce obsahu ve vstupu.
pub fn parse(lines: &[Line<'_>], end: usize) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();

    for line in lines {
        let Some(heading) = parse_heading(line.text) else {
            continue;
        };
        if let Some(prev) = out.last_mut() {
            prev.content_span.end = line.start;
        }
        out.push(Section {
            title_span: Span::new(
                line.abs(heading.title_rel),
                line.abs(heading.title_rel + heading.title.len()),
            ),
            title: heading.title,
            block_id: heading.block_id,
            span: line.span(),
            content_span: Span::new(line.end(), end),
            line: line.number,
            level: heading.level - 1,
        });
    }

    out
}
