//! Kontrola překlepů.
//!
//! Nepoužíváme vlastní slovník ani vlastní implementaci — zapínáme kontrolu
//! zabudovanou ve webview, která sahá po systémových slovnících (enchant /
//! hunspell). Uživatel tak má stejnou češtinu jako ve zbytku systému a
//! aplikace s sebou nemusí tahat slovníková data.

use std::fs;
use std::path::Path;

/// Složky, kam Debian a spol. instalují hunspell slovníky.
const DICT_DIRS: [&str; 3] = [
    "/usr/share/hunspell",
    "/usr/share/myspell/dicts",
    "/usr/local/share/hunspell",
];

/// Slovníky dostupné v systému, například `["cs_CZ", "en_US"]`.
///
/// Seznam je jen pro nabídku v nastavení; když si uživatel zvolí jazyk, který
/// tu není, webview ho prostě ignoruje.
pub fn available_dictionaries() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for dir in DICT_DIRS {
        let Ok(entries) = fs::read_dir(Path::new(dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "dic").unwrap_or(false) {
                if let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) {
                    if !out.contains(&stem) {
                        out.push(stem);
                    }
                }
            }
        }
    }

    out.sort();
    out
}

/// Zapne nebo vypne kontrolu překlepů v okně.
#[cfg(target_os = "linux")]
pub fn apply(window: &tauri::WebviewWindow, enabled: bool, languages: Vec<String>) -> Result<(), String> {
    use webkit2gtk::{WebContextExt, WebViewExt};

    window
        .with_webview(move |webview| {
            let Some(context) = webview.inner().context() else {
                return;
            };
            if enabled && !languages.is_empty() {
                let refs: Vec<&str> = languages.iter().map(String::as_str).collect();
                context.set_spell_checking_languages(&refs);
            }
            context.set_spell_checking_enabled(enabled);
        })
        .map_err(|e| format!("nelze nastavit kontrolu překlepů: {e}"))
}

/// Na ostatních platformách si kontrolu řídí webview samo podle atributu
/// `spellcheck` v HTML, takže tu není co nastavovat.
#[cfg(not(target_os = "linux"))]
pub fn apply(
    _window: &tauri::WebviewWindow,
    _enabled: bool,
    _languages: Vec<String>,
) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::available_dictionaries;

    #[test]
    fn seznam_slovniku_je_serazeny_a_bez_duplicit() {
        let dicts = available_dictionaries();
        let mut sorted = dicts.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(dicts, sorted);
    }

    #[test]
    fn nazvy_slovniku_nemaji_priponu() {
        for dict in available_dictionaries() {
            assert!(!dict.ends_with(".dic"), "{dict} má zůstat bez přípony");
        }
    }
}
