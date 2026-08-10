//! Parser nesmí spadnout na žádném vstupu.
//!
//! Poznámky nemusí psát jen tahle aplikace — můžou přijít z gitu, ze
//! Syncthingu nebo od někoho jiného. Panika v parseru by shodila celé okno,
//! takže sem patří všechno, co vypadá nepříjemně.

use textura_core::parse;

/// Rozparsuje vstup a projde všechny výsledky, aby se ověřilo, že spany
/// ukazují na platné hranice znaků.
fn projdi(input: &str) {
    let note = parse(input);

    for link in &note.links {
        let _ = link.span.slice(input);
        let _ = link.display_text();
    }
    for tag in &note.tags {
        assert_eq!(tag.span.slice(input), format!("#{}", tag.name));
    }
    for section in &note.sections {
        assert_eq!(section.title_span.slice(input), section.title);
        let _ = section.content_span.slice(input);
    }
    for task in &note.tasks {
        assert_eq!(task.text_span.slice(input), task.text);
        let _ = task.marker_span.slice(input);
    }
    for url in &note.urls {
        assert_eq!(url.span.slice(input), url.url);
    }
    for comment in &note.comments {
        let _ = comment.span.slice(input);
    }
    for property in &note.properties {
        assert_eq!(property.key_span.slice(input), property.key);
        assert_eq!(property.value_span.slice(input), property.value);
    }
    let _ = note.all_tags();
}

#[test]
fn neuzavrene_a_zpola_zapsane_konstrukce() {
    for input in [
        "[[",
        "]]",
        "[[]]",
        "[[|]]",
        "[[#]]",
        "[[^]]",
        "[[|||]]",
        "[[###]]",
        "![[",
        "!![[x]]",
        "[[[[x]]]]",
        "%%",
        "%%%",
        "%%%%",
        "%% [[ %%",
        "==",
        "===",
        "== ==",
        "==x",
        "^",
        "^^^",
        "#",
        "##",
        "#/",
        "#-",
        "@",
        "@:",
        "@:x",
        "[",
        "[]",
        "[ ",
        "[ ]",
        "[x]",
        "- [",
        "https://",
        "http://",
        "https:// ",
        "\u{feff}",
        "\u{feff}\u{feff}",
        "\r",
        "\r\n",
        "\n\r",
    ] {
        projdi(input);
    }
}

#[test]
fn opakovani_a_dlouhe_vstupy() {
    let cases = [
        "[[".repeat(5_000),
        "]]".repeat(5_000),
        "[[x]]".repeat(5_000),
        "%%".repeat(5_000),
        "#".repeat(5_000),
        "=".repeat(5_000),
        "@".repeat(5_000),
        format!("[[{}]]", "a".repeat(50_000)),
        format!("{}\n", "== x ==".repeat(1_000)),
        format!("#{}", "a/".repeat(10_000)),
        "\n".repeat(50_000),
        "\r\n".repeat(25_000),
        format!("@title: {}", "x".repeat(100_000)),
    ];
    for input in &cases {
        projdi(input);
    }
}

#[test]
fn unicode_a_hranice_znaku() {
    for input in [
        "🦀",
        "[[🦀]]",
        "#🦀",              // emoji není písmeno, tag nevznikne
        "#č🦀",
        "== 🦀 ==",
        "[ ] 🦀 @due:🦀",
        "%% 🦀 %%",
        "https://🦀.example",
        "ﬁ",                 // ligatura, to_lowercase mění délku
        "İ",                 // turecké I, to_lowercase je delší
        "ẞ",                 // velké ostré s
        "[[İstanbul]]",
        "#İstanbul",
        "== İ ==",
        "a\u{0301}",         // kombinující diakritika
        "\u{200b}[[x]]",     // zero width space
        "\u{feff}@title: x", // BOM
        "x\u{feff}[[y]]",    // BOM uprostřed
    ] {
        projdi(input);
    }
}

#[test]
fn poskozena_utf8_data_z_disku() {
    // Tohle simuluje, co udělá Workspace::scan se souborem, který není
    // validní UTF-8 — načítá se přes from_utf8_lossy.
    let raw: Vec<u8> = vec![
        0xff, 0xfe, b'[', b'[', 0xc3, b'x', b']', b']', 0x00, b'#', 0xed, 0xa0, 0x80, b'a', b'\n',
    ];
    let text = String::from_utf8_lossy(&raw);
    projdi(&text);
}

#[test]
fn pseudonahodne_vstupy() {
    // Jednoduchý deterministický generátor — bez závislosti na fuzz crate.
    let alphabet: Vec<char> = "[]#@%^=|/\\ \n\r\tabč🦀.:-!x".chars().collect();
    let mut seed: u64 = 0x5eed_1234_9abc_def0;

    for _ in 0..3_000 {
        let mut input = String::new();
        let len = {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize % 120
        };
        for _ in 0..len {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            input.push(alphabet[(seed >> 33) as usize % alphabet.len()]);
        }
        projdi(&input);
    }
}
