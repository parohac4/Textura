//! Převod byte offsetů (Rust) na UTF-16 offsety (JavaScript).
//!
//! Parser vrací byte rozsahy do UTF-8 textu, ale CodeMirror i celý JS pracují
//! s UTF-16 indexy. Bez převodu by se u diakritiky a emoji rozjelo zvýrazňování
//! i pozice kurzoru — což je přesně ta část formátu, na které nám záleží.

/// Mapa hranic znaků: pro každý znak dvojice (byte offset, UTF-16 offset).
pub struct Utf16Map {
    boundaries: Vec<(usize, u32)>,
    len_utf16: u32,
}

impl Utf16Map {
    pub fn new(text: &str) -> Self {
        let mut boundaries = Vec::with_capacity(text.len() / 2 + 1);
        let mut utf16 = 0u32;
        for (byte, ch) in text.char_indices() {
            boundaries.push((byte, utf16));
            utf16 += ch.len_utf16() as u32;
        }
        Self {
            boundaries,
            len_utf16: utf16,
        }
    }

    /// UTF-16 offset odpovídající byte offsetu. Offsety mimo text se ořežou
    /// na konec, aby chyba nikdy neshodila UI.
    pub fn at(&self, byte: usize) -> u32 {
        match self.boundaries.binary_search_by_key(&byte, |(b, _)| *b) {
            Ok(i) => self.boundaries[i].1,
            // Uvnitř znaku (nemělo by nastat) bereme jeho začátek.
            Err(0) => 0,
            Err(i) if i >= self.boundaries.len() => self.len_utf16,
            Err(i) => self.boundaries[i - 1].1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Utf16Map;

    #[test]
    fn ascii_offsety_sedi() {
        let map = Utf16Map::new("abc");
        assert_eq!(map.at(0), 0);
        assert_eq!(map.at(2), 2);
        assert_eq!(map.at(3), 3);
    }

    #[test]
    fn diakritika_posouva_byty_ale_ne_utf16() {
        let text = "Kůň [[Docker]]";
        let map = Utf16Map::new(text);
        let byte = text.find("[[").unwrap();
        assert_eq!(byte, 6); // ů a ň jsou dvoubytové
        assert_eq!(map.at(byte), 4);
    }

    #[test]
    fn emoji_zabira_dve_utf16_jednotky() {
        let text = "🦀 #tag";
        let map = Utf16Map::new(text);
        let byte = text.find('#').unwrap();
        assert_eq!(map.at(byte), 3); // emoji = 2 + mezera
    }

    #[test]
    fn offset_za_koncem_se_orizne() {
        let map = Utf16Map::new("ab");
        assert_eq!(map.at(99), 2);
    }
}
