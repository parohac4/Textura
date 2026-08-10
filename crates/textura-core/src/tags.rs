//! Tagy `#tag`, včetně hierarchických `#server/linux`.
//!
//! Tag musí začínat písmenem (Unicode), takže `#123` tag není, a `#` musí být
//! na začátku řádku nebo po oddělovači, takže `C#` tag také není.

use crate::text::{char_before, is_boundary_before, lines, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// Název bez `#`, v původním zápisu.
    pub name: String,
    /// Celý rozsah včetně `#`.
    pub span: Span,
    pub line: usize,
}

impl Tag {
    /// Části hierarchického tagu: `#server/linux` → `["server", "linux"]`.
    pub fn parts(&self) -> Vec<&str> {
        self.name.split('/').collect()
    }

    /// Klíč pro porovnání — tagy jsou case-insensitive.
    pub fn key(&self) -> String {
        self.name.to_lowercase()
    }
}

fn is_tag_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '/')
}

/// Ořízne koncové znaky, které jsou spíš interpunkcí než součástí tagu.
fn trim_tag(name: &str) -> &str {
    name.trim_end_matches(['/', '-', '_'])
}

/// Najde tagy mimo zadané vyloučené rozsahy (wiki odkazy, URL).
pub fn parse(content: &str, base: usize, exclude: &[Span]) -> Vec<Tag> {
    let line_index = lines(content, base);
    let mut out = Vec::new();
    let mut cursor = 0usize;

    while let Some(rel) = content[cursor..].find('#') {
        let hash = cursor + rel;
        cursor = hash + 1;

        // `#` musí být na začátku řádku nebo po oddělovači.
        if let Some(prev) = char_before(content, hash) {
            if !is_boundary_before(prev) {
                continue;
            }
        }

        let rest = &content[hash + 1..];
        let mut first = rest.chars();
        match first.next() {
            // První znak musí být písmeno — vyloučí `#123` i `#/x`.
            Some(ch) if ch.is_alphabetic() => {}
            _ => continue,
        }

        let raw_len = rest
            .char_indices()
            .find(|(_, ch)| !is_tag_char(*ch))
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let name = trim_tag(&rest[..raw_len]);
        if name.is_empty() {
            continue;
        }

        let span = Span::new(base + hash, base + hash + 1 + name.len());
        if exclude.iter().any(|ex| span.start >= ex.start && span.start < ex.end) {
            continue;
        }

        let line = line_index
            .iter()
            .rev()
            .find(|l| span.start >= l.start)
            .map(|l| l.number)
            .unwrap_or(1);

        out.push(Tag {
            name: name.to_string(),
            span,
            line,
        });
        cursor = hash + 1 + raw_len;
    }

    out
}
