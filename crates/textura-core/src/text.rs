//! Základní práce s textem poznámky: BOM, konce řádků, rozpad na řádky.

/// Byte rozsah v původním vstupu (včetně případného BOM).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    pub fn contains(&self, pos: usize) -> bool {
        pos >= self.start && pos < self.end
    }

    /// Leží celý rozsah uvnitř `other`?
    pub fn inside(&self, other: &Span) -> bool {
        self.start >= other.start && self.end <= other.end
    }

    /// Vrátí odpovídající výřez z původního vstupu.
    pub fn slice<'a>(&self, input: &'a str) -> &'a str {
        &input[self.start..self.end]
    }
}

/// Konce řádků nalezené v souboru. Při ukládání se má zachovat původní styl.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// `\n` — výchozí pro nové soubory.
    Lf,
    /// `\r\n`
    Crlf,
    /// Soubor obsahuje obojí; aplikace nesmí míchání sama „opravovat“.
    Mixed,
}

impl LineEnding {
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Crlf => "\r\n",
            // U smíšeného souboru je LF bezpečnější default pro nově psané řádky.
            LineEnding::Lf | LineEnding::Mixed => "\n",
        }
    }
}

/// Zjistí převládající konec řádku. Prázdný soubor je LF.
pub fn detect_line_ending(content: &str) -> LineEnding {
    let mut lf = 0usize;
    let mut crlf = 0usize;
    let bytes = content.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            if i > 0 && bytes[i - 1] == b'\r' {
                crlf += 1;
            } else {
                lf += 1;
            }
        }
    }
    match (lf, crlf) {
        (0, 0) => LineEnding::Lf,
        (0, _) => LineEnding::Crlf,
        (_, 0) => LineEnding::Lf,
        _ => LineEnding::Mixed,
    }
}

pub const BOM: char = '\u{feff}';

/// Jeden řádek bez ukončovacího `\n` / `\r\n`.
#[derive(Debug, Clone, Copy)]
pub struct Line<'a> {
    /// Text řádku bez konce řádku.
    pub text: &'a str,
    /// Absolutní byte offset začátku řádku ve vstupu.
    pub start: usize,
    /// Číslo řádku počítané od 1.
    pub number: usize,
}

impl<'a> Line<'a> {
    pub fn end(&self) -> usize {
        self.start + self.text.len()
    }

    pub fn span(&self) -> Span {
        Span::new(self.start, self.end())
    }

    /// Absolutní offset pozice `rel` uvnitř `self.text`.
    pub fn abs(&self, rel: usize) -> usize {
        self.start + rel
    }
}

/// Rozdělí obsah na řádky. `base` je offset `content` v původním vstupu.
pub fn lines(content: &str, base: usize) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    let mut offset = base;
    for (i, raw) in content.split_inclusive('\n').enumerate() {
        let text = raw.strip_suffix('\n').unwrap_or(raw);
        let text = text.strip_suffix('\r').unwrap_or(text);
        out.push(Line {
            text,
            start: offset,
            number: i + 1,
        });
        offset += raw.len();
    }
    out
}

/// Nahradí zadané rozsahy mezerami se zachováním byte offsetů.
///
/// Každý znak uvnitř rozsahu se nahradí tolika mezerami, kolik měl bytů, takže
/// maskovaný řetězec má stejnou délku i stejné byte pozice jako originál. Díky
/// tomu můžou další fáze parseru pracovat nad maskou a spany získané z masky
/// platí beze změny i pro původní text.
pub fn mask_spans(content: &str, spans: &[Span], base: usize) -> String {
    if spans.is_empty() {
        return content.to_string();
    }
    let mut out = String::with_capacity(content.len());
    for (rel, ch) in content.char_indices() {
        let abs = base + rel;
        if spans.iter().any(|s| s.contains(abs)) {
            for _ in 0..ch.len_utf8() {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
    }
    debug_assert_eq!(out.len(), content.len());
    out
}

/// Je znak povolený jako oddělovač před tagem / URL (začátek řádku se řeší zvlášť)?
pub(crate) fn is_boundary_before(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '(' | '"' | '\'' | '„' | '»' | '<')
}

/// Znak předcházející pozici `pos` v `s`, pokud existuje.
pub(crate) fn char_before(s: &str, pos: usize) -> Option<char> {
    s[..pos].chars().next_back()
}
