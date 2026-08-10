//! Automatická detekce externích URL. URL se nikdy nepřevádí na jiný formát.

use crate::text::{char_before, is_boundary_before, lines, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub url: String,
    pub span: Span,
    pub line: usize,
}

const SCHEMES: [&str; 2] = ["https://", "http://"];

/// Interpunkce na konci věty do URL nepatří.
fn trim_trailing(url: &str) -> &str {
    let mut end = url.len();
    loop {
        let candidate = &url[..end];
        let Some(last) = candidate.chars().next_back() else {
            break;
        };
        let drop = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '“' | '”' | '…' => true,
            // Uzavírací závorku ubereme jen tehdy, když k ní nepatří otevírací.
            ')' => candidate.matches(')').count() > candidate.matches('(').count(),
            ']' => candidate.matches(']').count() > candidate.matches('[').count(),
            _ => false,
        };
        if !drop {
            break;
        }
        end -= last.len_utf8();
    }
    &url[..end]
}

/// Najde URL mimo zadané vyloučené rozsahy (typicky wiki odkazy).
pub fn parse(content: &str, base: usize, exclude: &[Span]) -> Vec<Url> {
    let line_index = lines(content, base);
    let mut out = Vec::new();
    let mut cursor = 0usize;

    while cursor < content.len() {
        let hit = SCHEMES
            .iter()
            .filter_map(|scheme| content[cursor..].find(scheme).map(|i| (cursor + i, *scheme)))
            .min_by_key(|(pos, _)| *pos);
        let Some((start, _scheme)) = hit else {
            break;
        };

        if let Some(prev) = char_before(content, start) {
            if !is_boundary_before(prev) {
                cursor = start + 1;
                continue;
            }
        }

        let rest = &content[start..];
        let raw_end = rest
            .char_indices()
            .find(|(_, ch)| ch.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let url = trim_trailing(&rest[..raw_end]);
        let span = Span::new(base + start, base + start + url.len());

        cursor = start + raw_end.max(1);

        if exclude.iter().any(|ex| span.start >= ex.start && span.start < ex.end) {
            continue;
        }
        // Samotné schéma bez hostitele URL není.
        if url.ends_with("//") {
            continue;
        }

        let line = line_index
            .iter()
            .rev()
            .find(|l| span.start >= l.start)
            .map(|l| l.number)
            .unwrap_or(1);

        out.push(Url {
            url: url.to_string(),
            span,
            line,
        });
    }

    out
}
