//! Nastavování properties nesmí poznámku přeformátovat.

use textura_core::properties;
use textura_core::parse;

#[test]
fn prida_property_do_poznamky_bez_bloku() {
    let out = properties::set("Docker\n\nText\n", "color", Some("blue"));
    assert_eq!(out, "@color: blue\n\nDocker\n\nText\n");
    assert_eq!(parse(&out).property("color").unwrap().value, "blue");
}

#[test]
fn prida_property_za_existujici_blok() {
    let out = properties::set("@title: Docker\n@status: active\n\nText\n", "color", Some("red"));
    assert_eq!(out, "@title: Docker\n@status: active\n@color: red\n\nText\n");
}

#[test]
fn zmeni_existujici_property() {
    let out = properties::set("@color: red\n@title: Docker\n\nText\n", "color", Some("green"));
    assert_eq!(out, "@color: green\n@title: Docker\n\nText\n");
}

#[test]
fn odstrani_property() {
    let out = properties::set("@title: Docker\n@color: red\n\nText\n", "color", None);
    assert_eq!(out, "@title: Docker\n\nText\n");
}

#[test]
fn odstraneni_neexistujici_property_nic_neudela() {
    let input = "Docker\n\nText\n";
    assert_eq!(properties::set(input, "color", None), input);
}

#[test]
fn klic_se_hleda_case_insensitive() {
    let out = properties::set("@Color: red\n\nText\n", "color", Some("blue"));
    assert_eq!(out, "@color: blue\n\nText\n");
}

#[test]
fn zachova_crlf() {
    let out = properties::set("@title: Docker\r\n\r\nText\r\n", "color", Some("red"));
    assert_eq!(out, "@title: Docker\r\n@color: red\r\n\r\nText\r\n");

    let novy = properties::set("Docker\r\n", "color", Some("red"));
    assert_eq!(novy, "@color: red\r\n\r\nDocker\r\n");
}

#[test]
fn zachova_bom() {
    let out = properties::set("\u{feff}Docker\n", "color", Some("red"));
    assert_eq!(out, "\u{feff}@color: red\n\nDocker\n");
    assert!(parse(&out).had_bom);
}

#[test]
fn prazdna_poznamka() {
    let out = properties::set("", "color", Some("red"));
    assert_eq!(out, "@color: red\n");
}

#[test]
fn poznamka_zacinajici_prazdnym_radkem_nedostane_dalsi() {
    let out = properties::set("\nDocker\n", "color", Some("red"));
    assert_eq!(out, "@color: red\n\nDocker\n");
}

#[test]
fn zbytek_poznamky_zustava_nedotceny() {
    let input = "@title: Docker\n\nDocker\n\n== Sekce ==\n\n[[Odkaz]] #tag\n[ ] úkol\n";
    let out = properties::set(input, "color", Some("purple"));
    let note = parse(&out);

    assert_eq!(note.property("color").unwrap().value, "purple");
    assert_eq!(note.sections.len(), 1);
    assert_eq!(note.links.len(), 1);
    assert_eq!(note.tags.len(), 1);
    assert_eq!(note.tasks.len(), 1);
    assert!(out.ends_with("[ ] úkol\n"));
}
