//! Properties poznámky: `@klic: hodnota` na začátku souboru.
//!
//! Pravidla (viz `docs/format.md`):
//!
//! * blok začíná na prvním řádku souboru (po případném BOM),
//! * tvoří ho souvislá řada řádků `@klic: hodnota`,
//! * blok končí prvním řádkem, který tomuto tvaru neodpovídá — včetně
//!   prázdného řádku,
//! * properties jsou zcela volitelné.
//!
//! Tvar `@klic:hodnota` uvnitř řádku (např. `[ ] Úkol @due:2026-08-15`) není
//! property; to je atribut úkolu, viz [`crate::tasks`].

use crate::text::{Line, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Klíč bez `@` a bez `:`, v původním zápisu.
    pub key: String,
    /// Hodnota ořezaná o okolní whitespace. Může být prázdná.
    pub value: String,
    pub key_span: Span,
    pub value_span: Span,
    /// Celý řádek.
    pub span: Span,
    pub line: usize,
}

impl Property {
    /// Klíč v malých písmenech — porovnání klíčů je case-insensitive.
    pub fn key_lower(&self) -> String {
        self.key.to_lowercase()
    }

    /// Hodnota rozpadlá podle čárek (pro `@aliases`, `@tags`).
    pub fn list_values(&self) -> Vec<String> {
        self.value
            .split(',')
            .map(|part| part.trim())
            .filter(|part| !part.is_empty())
            .map(|part| part.to_string())
            .collect()
    }
}

fn is_key_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.')
}

/// Rozparsuje jeden řádek jako property, pokud tvarem odpovídá.
fn parse_line(line: &Line<'_>) -> Option<Property> {
    let text = line.text;
    let rest = text.strip_prefix('@')?;

    let key_len = rest
        .char_indices()
        .find(|(_, ch)| !is_key_char(*ch))
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    if key_len == 0 {
        return None;
    }
    if !rest[key_len..].starts_with(':') {
        return None;
    }

    let key = &rest[..key_len];
    let key_start = 1; // za '@'
    let value_raw_start = key_start + key_len + 1; // za ':'
    let value_raw = &text[value_raw_start..];
    let lead = value_raw.len() - value_raw.trim_start().len();
    let value = value_raw.trim();

    Some(Property {
        key: key.to_string(),
        value: value.to_string(),
        key_span: Span::new(line.abs(key_start), line.abs(key_start + key_len)),
        value_span: Span::new(
            line.abs(value_raw_start + lead),
            line.abs(value_raw_start + lead + value.len()),
        ),
        span: line.span(),
        line: line.number,
    })
}

/// Nastaví, změní nebo odstraní property a vrátí nový obsah poznámky.
///
/// `None` property odstraní. Zbytek textu zůstává nedotčený včetně konců
/// řádků a BOM — poznámku upravuje i uživatel v jiném editoru a aplikace mu
/// ji nesmí přeformátovat.
pub fn set(content: &str, key: &str, value: Option<&str>) -> String {
    let note = crate::parse(content);
    let eol = note.line_ending.as_str();
    let mut out = content.to_string();

    match note.property(key) {
        Some(existing) => match value {
            Some(value) => {
                out.replace_range(existing.span.start..existing.span.end, &format!("@{key}: {value}"));
            }
            None => {
                // Odstraníme celý řádek včetně jeho konce.
                let end = line_end(content, existing.span.end);
                out.replace_range(existing.span.start..end, "");
            }
        },
        None => {
            let Some(value) = value else {
                return out;
            };
            let line = format!("@{key}: {value}");

            match note.properties.last() {
                // Za poslední existující property.
                Some(last) => {
                    let at = line_end(content, last.span.end);
                    out.insert_str(at, &format!("{line}{eol}"));
                }
                // Nový blok úplně na začátek, oddělený prázdným řádkem.
                None => {
                    let at = if note.had_bom {
                        crate::text::BOM.len_utf8()
                    } else {
                        0
                    };
                    let needs_blank = !content[at..].starts_with('\n')
                        && !content[at..].starts_with('\r')
                        && !content[at..].is_empty();
                    let separator = if needs_blank { eol } else { "" };
                    out.insert_str(at, &format!("{line}{eol}{separator}"));
                }
            }
        }
    }

    out
}

/// Offset za koncem řádku, který končí na `pos` (bez terminátoru).
fn line_end(content: &str, pos: usize) -> usize {
    let rest = &content[pos..];
    if rest.starts_with("\r\n") {
        pos + 2
    } else if rest.starts_with('\n') || rest.starts_with('\r') {
        pos + 1
    } else {
        pos
    }
}

/// Načte blok properties ze začátku souboru.
///
/// Vrací nalezené properties a číslo prvního řádku těla (1-based); pokud blok
/// chybí, je to řádek 1.
pub fn parse(lines: &[Line<'_>]) -> (Vec<Property>, usize) {
    let mut out = Vec::new();
    for line in lines {
        match parse_line(line) {
            Some(prop) => out.push(prop),
            None => return (out, line.number),
        }
    }
    (out, lines.len() + 1)
}
