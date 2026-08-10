//! Technické komentáře aplikace: `%% text %%`.
//!
//! Komentář může přesahovat přes více řádků. Neukládají se do něj kritická
//! metadata — ta patří do `.txtnote/`.

use crate::text::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// Celý rozsah včetně obou `%%`.
    pub span: Span,
    /// Rozsah vnitřního textu bez `%%` a bez okolních mezer.
    pub text_span: Span,
    /// Vnitřní text bez `%%`, ořezaný o okolní whitespace.
    pub text: String,
}

const MARK: &str = "%%";

/// Najde všechny uzavřené komentáře. Neuzavřené `%%` se ignoruje — poznámka
/// zůstává čitelná i tak a nechceme, aby jeden překlep „snědl“ zbytek souboru.
pub fn parse(content: &str, base: usize) -> Vec<Comment> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    while let Some(rel_open) = content[cursor..].find(MARK) {
        let open = cursor + rel_open;
        let inner_start = open + MARK.len();
        let Some(rel_close) = content[inner_start..].find(MARK) else {
            break;
        };
        let inner_end = inner_start + rel_close;
        let close_end = inner_end + MARK.len();

        let raw = &content[inner_start..inner_end];
        let trimmed = raw.trim();
        let lead = raw.len() - raw.trim_start().len();

        out.push(Comment {
            span: Span::new(base + open, base + close_end),
            text_span: Span::new(
                base + inner_start + lead,
                base + inner_start + lead + trimmed.len(),
            ),
            text: trimmed.to_string(),
        });

        cursor = close_end;
    }

    out
}
