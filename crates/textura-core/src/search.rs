//! Normalizace textu pro fulltext a rozpad dotazu na hledané výrazy.
//!
//! Hledání porovnává *složený* (folded) text: malá písmena a bez diakritiky,
//! takže `zluty` najde `Žluťoučký` a `Manana` najde `mañana`. Uživatel, který
//! píše česky, tak nemusí přepínat klávesnici, aby si našel vlastní poznámku.
//!
//! Skládání je záměrně bez závislostí (jádro nesmí mít žádné) — místo plné
//! unicodové normalizace je to tabulka latinkových písmen s diakritikou plus
//! zahození kombinujících znamének, takže se poradí i s textem v NFD.

/// Složí znak pro hledání: malá písmena bez diakritiky.
///
/// Vrací iterátor, protože `to_lowercase` může z jednoho znaku udělat víc
/// (`İ` → `i` + kombinující tečka) a kombinující znaménka se zahazují, takže
/// znak se může složit i na nic.
pub fn fold_char(ch: char) -> impl Iterator<Item = char> {
    ch.to_lowercase().filter_map(strip_diacritics)
}

/// Složí celý text pro hledání.
pub fn fold(s: &str) -> String {
    s.chars().flat_map(fold_char).collect()
}

/// Složí text a k tomu vrátí mapu offsetů zpátky do originálu.
///
/// Skládání mění délku i počet znaků, takže přepočet přes počty znaků by
/// rozsah nálezu posunul; mapa drží pro každý byte výsledku byte offset znaku,
/// ze kterého vznikl. Mapa má o jeden prvek navíc jako zarážku, aby šel
/// adresovat i konec.
pub fn fold_with_offsets(s: &str) -> (String, Vec<usize>) {
    let mut folded = String::with_capacity(s.len());
    let mut map: Vec<usize> = Vec::with_capacity(s.len() + 1);

    for (byte, ch) in s.char_indices() {
        let before = folded.len();
        for fc in fold_char(ch) {
            folded.push(fc);
        }
        map.resize(map.len() + (folded.len() - before), byte);
    }
    map.push(s.len());

    (folded, map)
}

/// Dotaz rozpadlý na jednotlivé výrazy, každý už složený pro porovnávání.
///
/// Výrazy oddělené mezerami platí jako AND: `docker mariadb` najde poznámky,
/// ve kterých je obojí — na pořadí ani na tom, jestli jsou na stejném řádku,
/// nezáleží.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    terms: Vec<String>,
}

impl Query {
    /// Rozpadne dotaz na výrazy. Prázdné výrazy zmizí, takže samá mezera nebo
    /// text, ze kterého po složení nic nezbyde, dá prázdný dotaz.
    pub fn parse(query: &str) -> Self {
        let terms = query
            .split_whitespace()
            .map(fold)
            .filter(|t| !t.is_empty())
            .collect();
        Self { terms }
    }

    /// Prázdný dotaz nehledá nic — volající nemá co vracet.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    /// Obsahuje už složený text všechny výrazy dotazu?
    pub fn matches_folded(&self, folded: &str) -> bool {
        !self.is_empty() && self.terms.iter().all(|t| folded.contains(t.as_str()))
    }

    /// Obsahuje text všechny výrazy dotazu? Text se složí sám.
    pub fn matches(&self, text: &str) -> bool {
        self.matches_folded(&fold(text))
    }

    /// První výskyt kteréhokoli výrazu ve složeném textu — rozsah pro
    /// zvýraznění nálezu. Nejdřív vyhrává pozice, při shodě delší výraz.
    pub fn first_hit_folded(&self, folded: &str) -> Option<(usize, usize)> {
        self.terms
            .iter()
            .filter_map(|t| folded.find(t.as_str()).map(|at| (at, at + t.len())))
            .min_by_key(|(start, end)| (*start, usize::MAX - (end - start)))
    }
}

/// Latinkové písmeno bez diakritiky; kombinující znaménko se zahodí.
///
/// Vstup je vždy už malé písmeno (viz [`fold_char`]), takže velká varianta
/// v tabulce být nemusí.
fn strip_diacritics(ch: char) -> Option<char> {
    // Kombinující diakritická znaménka — text v NFD se tím složí stejně
    // jako v NFC.
    if matches!(ch, '\u{0300}'..='\u{036f}' | '\u{fe20}'..='\u{fe2f}') {
        return None;
    }

    Some(match ch {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'č' | 'ĉ' | 'ċ' => 'c',
        'ď' | 'đ' | 'ð' => 'd',
        'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' | 'ħ' => 'h',
        'í' | 'ì' | 'î' | 'ï' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ł' => 'l',
        'ń' | 'ņ' | 'ň' | 'ñ' => 'n',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ō' | 'ŏ' | 'ő' | 'ø' => 'o',
        'ŕ' | 'ř' => 'r',
        'ś' | 'š' | 'ş' | 'ŝ' | 'ș' => 's',
        'ţ' | 'ť' | 'ț' | 'þ' => 't',
        'ú' | 'ù' | 'û' | 'ü' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ž' | 'ż' => 'z',
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::{fold, fold_with_offsets, Query};

    #[test]
    fn skladani_zahodi_diakritiku_i_velka_pismena() {
        assert_eq!(fold("Příliš Žluťoučký KŮŇ"), "prilis zlutoucky kun");
        // `ß` zůstává — nemá základní latinkovou podobu jako jeden znak.
        assert_eq!(fold("Mañana, Grüße"), "manana, gruße");
    }

    #[test]
    fn skladani_zvladne_i_nfd() {
        // `é` zapsané jako `e` + kombinující akut.
        assert_eq!(fold("e\u{0301}ter"), "eter");
    }

    #[test]
    fn mapa_offsetu_ukazuje_na_puvodni_znaky() {
        let text = "Žluťoučký kůň";
        let (folded, map) = fold_with_offsets(text);
        let at = folded.find("kun").unwrap();
        assert_eq!(&text[map[at]..map[at + 3]], "kůň");
    }

    #[test]
    fn mapa_drzi_offsety_i_kdyz_skladani_meni_pocet_znaku() {
        // Turecké İ se v lowercase rozpadne na `i` a kombinující tečku, která
        // se pak zahodí — počet znaků se tím mění oběma směry.
        let text = "İstanbul a potom docker";
        let (folded, map) = fold_with_offsets(text);
        assert_eq!(folded, "istanbul a potom docker");
        let at = folded.find("docker").unwrap();
        assert_eq!(&text[map[at]..map[at + 6]], "docker");
    }

    #[test]
    fn dotaz_se_rozpadne_na_vyrazy_a_slozi() {
        let q = Query::parse("  Docker   MariaDB ");
        assert_eq!(q.terms(), ["docker", "mariadb"]);
        assert!(!q.is_empty());
    }

    #[test]
    fn prazdny_dotaz_nic_nenajde() {
        assert!(Query::parse("   ").is_empty());
        // Ze samotného kombinujícího znaménka po složení nezbyde nic.
        assert!(Query::parse("\u{0301}").is_empty());
        assert!(!Query::parse("").matches("cokoli"));
    }

    #[test]
    fn vyrazy_plati_jako_and() {
        let q = Query::parse("docker mariadb");
        assert!(q.matches("MariaDB vedle Dockeru"));
        assert!(!q.matches("jen docker"));
    }

    #[test]
    fn prvni_nalez_bere_nejdrivejsi_pozici_a_pri_shode_delsi_vyraz() {
        let q = Query::parse("dock docker");
        assert_eq!(q.first_hit_folded("mam docker doma"), Some((4, 10)));
    }
}
