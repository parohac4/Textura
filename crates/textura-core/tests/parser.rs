//! Testy formátu poznámek podle `docs/format.md`.

use textura_core::{parse, Anchor, LineEnding, LinkKind, TaskState};

// --- properties ------------------------------------------------------------

#[test]
fn properties_se_ctou_ze_zacatku_souboru() {
    let note = parse("@title: Docker\n@status: active\n\nDocker\n");
    assert_eq!(note.properties.len(), 2);
    assert_eq!(note.title(), Some("Docker"));
    assert_eq!(note.property("status").unwrap().value, "active");
    assert_eq!(note.body_line, 3);
}

#[test]
fn poznamka_muze_byt_uplne_bez_properties() {
    let note = parse("Docker\n\n[[MariaDB]]\n");
    assert!(note.properties.is_empty());
    assert_eq!(note.title(), None);
    assert_eq!(note.body_line, 1);
}

#[test]
fn blok_properties_konci_prvnim_neodpovidajicim_radkem() {
    // `@` uprostřed textu už do bloku nepatří.
    let note = parse("@title: Docker\n\n@ale tohle je věta\n@status: active\n");
    assert_eq!(note.properties.len(), 1);
    assert_eq!(note.property("status"), None);
}

#[test]
fn libovolne_uzivatelske_properties() {
    let note = parse("@customer: ACME\n@priority: 1\n@url: https://example.com\n");
    assert_eq!(note.property("customer").unwrap().value, "ACME");
    assert_eq!(note.property("priority").unwrap().value, "1");
    assert_eq!(note.property("url").unwrap().value, "https://example.com");
}

#[test]
fn klice_properties_jsou_case_insensitive() {
    let note = parse("@Title: Docker\n");
    assert_eq!(note.title(), Some("Docker"));
}

#[test]
fn prazdna_hodnota_property_je_povolena() {
    let note = parse("@status:\n\nText\n");
    assert_eq!(note.properties.len(), 1);
    assert_eq!(note.property("status").unwrap().value, "");
}

#[test]
fn aliasy_se_deli_carkou() {
    let note = parse("@aliases: Docker Engine, Containers\n");
    assert_eq!(note.aliases(), vec!["Docker Engine", "Containers"]);
}

#[test]
fn property_spany_ukazuji_na_puvodni_text() {
    let input = "@title: Docker\n";
    let note = parse(input);
    let prop = &note.properties[0];
    assert_eq!(prop.key_span.slice(input), "title");
    assert_eq!(prop.value_span.slice(input), "Docker");
}

// --- wiki odkazy -----------------------------------------------------------

#[test]
fn jednoduchy_odkaz() {
    let note = parse("[[Docker]]\n");
    let link = &note.links[0];
    assert_eq!(link.target, "Docker");
    assert_eq!(link.kind, LinkKind::Note);
    assert!(link.label.is_none());
    assert!(!link.embed);
    assert_eq!(link.display_text(), "Docker");
}

#[test]
fn odkaz_s_cestou() {
    let note = parse("[[Projects/Server]]\n");
    let link = &note.links[0];
    assert_eq!(link.target, "Projects/Server");
    assert_eq!(link.explicit_path().as_deref(), Some("Projects/Server.txt"));
}

#[test]
fn odkaz_s_aliasem_oddeluje_cil_a_popisek() {
    let note = parse("[[Docker|kontejnerová platforma]]\n");
    let link = &note.links[0];
    assert_eq!(link.target, "Docker");
    assert_eq!(link.label.as_deref(), Some("kontejnerová platforma"));
    assert_eq!(link.display_text(), "kontejnerová platforma");
}

#[test]
fn odkaz_na_sekci() {
    let note = parse("[[Docker#Instalace]]\n");
    let link = &note.links[0];
    assert_eq!(link.target, "Docker");
    assert_eq!(link.anchor, Some(Anchor::Section("Instalace".into())));
}

#[test]
fn odkaz_na_sekci_s_aliasem() {
    let note = parse("[[Docker#Instalace|instalace Dockeru]]\n");
    let link = &note.links[0];
    assert_eq!(link.target, "Docker");
    assert_eq!(link.anchor, Some(Anchor::Section("Instalace".into())));
    assert_eq!(link.label.as_deref(), Some("instalace Dockeru"));
}

#[test]
fn odkaz_na_stabilni_id() {
    let note = parse("[[Docker#^docker-install]]\n");
    let link = &note.links[0];
    assert_eq!(link.target, "Docker");
    assert_eq!(link.anchor, Some(Anchor::BlockId("docker-install".into())));
}

#[test]
fn odkaz_na_prilohu_ma_priponu() {
    let note = parse("[[attachments/manual.pdf]]\n[[attachments/server-rack.jpg]]\n");
    assert_eq!(note.links[0].kind, LinkKind::Attachment);
    assert_eq!(note.links[0].target, "attachments/manual.pdf");
    assert_eq!(note.links[1].kind, LinkKind::Attachment);
    assert_eq!(note.attachments().count(), 2);
    assert_eq!(note.note_links().count(), 0);
}

#[test]
fn tecka_v_nazvu_poznamky_neni_pripona() {
    let note = parse("[[Verze 1.2]]\n");
    assert_eq!(note.links[0].kind, LinkKind::Note);
    assert_eq!(note.links[0].target, "Verze 1.2");
}

#[test]
fn napsana_pripona_txt_se_odstrani() {
    let note = parse("[[Docker.txt]]\n");
    assert_eq!(note.links[0].kind, LinkKind::Note);
    assert_eq!(note.links[0].target, "Docker");
}

#[test]
fn embed_poznamky() {
    let input = "![[Docker]]\n";
    let note = parse(input);
    let link = &note.links[0];
    assert!(link.embed);
    assert_eq!(link.target, "Docker");
    assert_eq!(link.span.slice(input), "![[Docker]]");
}

#[test]
fn odkaz_jen_na_vlastni_sekci() {
    let note = parse("[[#Instalace]]\n");
    let link = &note.links[0];
    assert!(link.is_internal());
    assert_eq!(link.target, "");
    assert_eq!(link.display_text(), "Instalace");
}

#[test]
fn neuzavreny_odkaz_se_ignoruje() {
    let note = parse("[[Docker\n[[MariaDB]]\n");
    assert_eq!(note.links.len(), 1);
    assert_eq!(note.links[0].target, "MariaDB");
}

#[test]
fn odkaz_nepresahuje_pres_konec_radku() {
    let note = parse("[[Docker\nnějaký text]]\n");
    assert!(note.links.is_empty());
}

#[test]
fn spany_odkazu_ukazuji_na_puvodni_text() {
    let input = "Viz [[Docker|kontejnery]] a konec.\n";
    let note = parse(input);
    assert_eq!(note.links[0].span.slice(input), "[[Docker|kontejnery]]");
    assert_eq!(note.links[0].line, 1);
}

// --- sekce -----------------------------------------------------------------

#[test]
fn sekce_se_najdou_a_zachovaji_zapis() {
    let note = parse("Docker\n\n== Instalace ==\n\nPostup...\n\n== Networking ==\n\nSítě...\n");
    assert_eq!(note.sections.len(), 2);
    assert_eq!(note.sections[0].title, "Instalace");
    assert_eq!(note.sections[1].title, "Networking");
    assert_eq!(note.sections[0].level, 1);
}

#[test]
fn sekce_se_dohledava_case_insensitive() {
    let note = parse("== Instalace ==\n");
    assert!(note.section("instalace").is_some());
    assert!(note.section("INSTALACE").is_some());
    // ale původní zápis zůstává zachovaný
    assert_eq!(note.section("instalace").unwrap().title, "Instalace");
}

#[test]
fn obsah_sekce_konci_dalsim_nadpisem() {
    let input = "== A ==\nprvní\n== B ==\ndruhý\n";
    let note = parse(input);
    assert_eq!(note.sections[0].content_span.slice(input), "\nprvní\n");
    assert_eq!(note.sections[1].content_span.slice(input), "\ndruhý\n");
}

#[test]
fn stabilni_id_sekce() {
    let note = parse("== Instalace == ^docker-install\n");
    let section = &note.sections[0];
    assert_eq!(section.title, "Instalace");
    assert_eq!(section.block_id.as_deref(), Some("docker-install"));
    assert!(note.section_by_id("docker-install").is_some());
}

#[test]
fn bezna_veta_s_rovnitky_neni_nadpis() {
    let note = parse("V configu je a == b a nic víc.\n== ==\n==\n");
    assert!(note.sections.is_empty());
}

#[test]
fn vnorena_sekce_ma_vyssi_uroven() {
    let note = parse("== A ==\n=== B ===\n");
    assert_eq!(note.sections[0].level, 1);
    assert_eq!(note.sections[1].level, 2);
}

// --- tagy ------------------------------------------------------------------

#[test]
fn zakladni_tagy() {
    let note = parse("#linux\n#server2026\n#fotografie\n");
    let names: Vec<_> = note.tags.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["linux", "server2026", "fotografie"]);
}

#[test]
fn hierarchicke_tagy() {
    let note = parse("#server/linux #projekt/mautic\n");
    assert_eq!(note.tags[0].name, "server/linux");
    assert_eq!(note.tags[0].parts(), vec!["server", "linux"]);
    assert_eq!(note.tags[1].name, "projekt/mautic");
}

#[test]
fn cislo_za_mrizkou_neni_tag() {
    let note = parse("#123 a #1abc\n");
    assert!(note.tags.is_empty());
}

#[test]
fn c_ostre_neni_tag() {
    let note = parse("Píšu v C# a v Rustu.\n");
    assert!(note.tags.is_empty());
}

#[test]
fn fragment_url_neni_tag() {
    let note = parse("https://example.com#kotva\n");
    assert!(note.tags.is_empty());
    assert_eq!(note.urls.len(), 1);
}

#[test]
fn tag_uvnitr_odkazu_se_neparsuje() {
    let note = parse("[[Docker#Instalace]]\n");
    assert!(note.tags.is_empty());
}

#[test]
fn tag_s_diakritikou() {
    let note = parse("#poznámky #čeština\n");
    assert_eq!(note.tags[0].name, "poznámky");
    assert_eq!(note.tags[1].name, "čeština");
}

#[test]
fn koncova_interpunkce_do_tagu_nepatri() {
    let input = "Dnes jen #linux, zítra #server.\n";
    let note = parse(input);
    assert_eq!(note.tags[0].name, "linux");
    assert_eq!(note.tags[0].span.slice(input), "#linux");
    assert_eq!(note.tags[1].name, "server");
}

#[test]
fn tagy_z_property_a_z_tela_se_slouci_bez_duplicit() {
    let note = parse("@tags: docker, server\n\nText\n\n#docker\n#linux\n");
    assert_eq!(note.all_tags(), vec!["docker", "server", "linux"]);
}

// --- úkoly -----------------------------------------------------------------

#[test]
fn checkboxy() {
    let note = parse("[ ] Aktualizovat server\n[x] Vytvořit zálohu\n[!] Důležitý úkol\n");
    assert_eq!(note.tasks.len(), 3);
    assert_eq!(note.tasks[0].state, TaskState::Open);
    assert_eq!(note.tasks[0].text, "Aktualizovat server");
    assert_eq!(note.tasks[1].state, TaskState::Done);
    assert!(note.tasks[1].state.is_done());
    assert_eq!(note.tasks[2].state, TaskState::Important);
}

#[test]
fn ukol_s_datem() {
    let note = parse("[ ] Aktualizovat server @due:2026-08-15\n");
    let task = &note.tasks[0];
    assert_eq!(task.text, "Aktualizovat server @due:2026-08-15");
    assert_eq!(task.attributes.len(), 1);
    assert_eq!(task.attributes[0].key, "due");
    assert_eq!(task.attributes[0].value, "2026-08-15");
}

#[test]
fn atribut_ukolu_neni_property_poznamky() {
    let note = parse("[ ] Úkol @due:2026-08-15\n");
    assert!(note.properties.is_empty());
}

#[test]
fn ukol_muze_byt_polozkou_seznamu() {
    let note = parse("- [ ] Úkol\n  [x] Odsazený úkol\n");
    assert_eq!(note.tasks.len(), 2);
    assert_eq!(note.tasks[1].indent, 2);
}

#[test]
fn odkaz_na_zacatku_radku_neni_ukol() {
    let note = parse("[[Docker]]\n");
    assert!(note.tasks.is_empty());
}

#[test]
fn marker_span_ukazuje_na_checkbox() {
    let input = "[ ] Úkol\n";
    let note = parse(input);
    assert_eq!(note.tasks[0].marker_span.slice(input), "[ ]");
}

// --- URL -------------------------------------------------------------------

#[test]
fn url_se_rozpozna_a_nemeni() {
    let input = "Viz https://mautic.org a http://example.com/a?b=1\n";
    let note = parse(input);
    assert_eq!(note.urls[0].url, "https://mautic.org");
    assert_eq!(note.urls[1].url, "http://example.com/a?b=1");
    assert_eq!(note.urls[0].span.slice(input), "https://mautic.org");
}

#[test]
fn tecka_na_konci_vety_neni_soucasti_url() {
    let note = parse("Viz https://example.com.\n");
    assert_eq!(note.urls[0].url, "https://example.com");
}

#[test]
fn url_v_zavorce() {
    let note = parse("(https://example.com/a_(b))\n");
    assert_eq!(note.urls[0].url, "https://example.com/a_(b)");
}

// --- komentáře -------------------------------------------------------------

#[test]
fn komentar_aplikace() {
    let note = parse("Text %% technický komentář %% dál\n");
    assert_eq!(note.comments.len(), 1);
    assert_eq!(note.comments[0].text, "technický komentář");
}

#[test]
fn obsah_komentare_se_neparsuje() {
    let note = parse("%% [[Docker]] #tag [ ] úkol %%\n");
    assert!(note.links.is_empty());
    assert!(note.tags.is_empty());
    assert!(note.tasks.is_empty());
}

#[test]
fn viceradkovy_komentar() {
    let note = parse("%% první\ndruhý %%\n[[Docker]]\n");
    assert_eq!(note.comments.len(), 1);
    assert_eq!(note.links.len(), 1);
}

// --- encoding a konce řádků ------------------------------------------------

#[test]
fn crlf_se_nacte_a_zapamatuje() {
    let note = parse("@title: Docker\r\n\r\nDocker\r\n\r\n#docker\r\n");
    assert_eq!(note.line_ending, LineEnding::Crlf);
    assert_eq!(note.title(), Some("Docker"));
    assert_eq!(note.tags[0].name, "docker");
}

#[test]
fn lf_je_vychozi() {
    assert_eq!(parse("a\nb\n").line_ending, LineEnding::Lf);
    assert_eq!(parse("").line_ending, LineEnding::Lf);
}

#[test]
fn smiseny_soubor_se_pozna() {
    assert_eq!(parse("a\r\nb\n").line_ending, LineEnding::Mixed);
}

#[test]
fn bom_se_pozna_a_neposune_spany() {
    let input = "\u{feff}@title: Docker\n";
    let note = parse(input);
    assert!(note.had_bom);
    assert_eq!(note.title(), Some("Docker"));
    assert_eq!(note.properties[0].key_span.slice(input), "title");
}

#[test]
fn emoji_a_diakritika_nerozbiji_offsety() {
    let input = "Příliš 🦀 žluťoučký [[Kůň]] #úpěl\n";
    let note = parse(input);
    assert_eq!(note.links[0].span.slice(input), "[[Kůň]]");
    assert_eq!(note.tags[0].span.slice(input), "#úpěl");
}

#[test]
fn prazdny_soubor_je_platna_poznamka() {
    let note = parse("");
    assert!(note.properties.is_empty());
    assert!(note.links.is_empty());
    assert!(note.tags.is_empty());
}

// --- kompletní příklad ze specifikace --------------------------------------

#[test]
fn kompletni_priklad_ze_specifikace() {
    let input = include_str!("fixtures/mautic.txt");
    let note = parse(input);

    assert_eq!(note.title(), Some("Produkční Mautic server"));
    assert_eq!(
        note.aliases(),
        vec!["Mautic Production", "Production Mautic"]
    );
    assert_eq!(note.property("created").unwrap().value, "2026-08-10");
    assert_eq!(note.property("status").unwrap().value, "active");

    let sections: Vec<_> = note.sections.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        sections,
        vec!["Infrastruktura", "Úkoly", "Dokumentace"]
    );

    let targets: Vec<_> = note.note_links().map(|l| l.target.as_str()).collect();
    assert_eq!(
        targets,
        vec!["Debian", "Apache", "MariaDB", "Docker", "Docker"]
    );

    let networking = note.links.iter().find(|l| l.label.is_some()).unwrap();
    assert_eq!(
        networking.anchor,
        Some(Anchor::Section("Networking".into()))
    );
    assert_eq!(networking.display_text(), "Docker networking");

    let attachments: Vec<_> = note.attachments().map(|l| l.target.as_str()).collect();
    assert_eq!(attachments, vec!["attachments/mautic-architecture.pdf"]);

    assert_eq!(note.tasks.len(), 3);
    assert_eq!(
        note.tasks.iter().filter(|t| t.state.is_done()).count(),
        1
    );

    assert_eq!(note.urls.len(), 1);
    assert_eq!(note.urls[0].url, "https://mautic.org");

    assert_eq!(note.all_tags(), vec!["mautic", "server", "production"]);
    assert!(note.comments.is_empty());
}

#[test]
fn poznamka_bez_metadat() {
    let input = include_str!("fixtures/plain.txt");
    let note = parse(input);

    assert!(note.properties.is_empty());
    assert!(note.sections.is_empty());
    assert_eq!(note.note_links().count(), 3);
    assert_eq!(note.all_tags(), vec!["docker", "server"]);
}

// --- invariant formátu -----------------------------------------------------

#[test]
fn parser_nikdy_nemeni_text_poznamky() {
    // Všechny spany musí ukazovat do původního vstupu — poznámka se nikde
    // nepřepisuje ani nenormalizuje.
    let input = include_str!("fixtures/mautic.txt");
    let note = parse(input);

    for link in &note.links {
        assert!(link.span.slice(input).contains("[["));
    }
    for tag in &note.tags {
        assert_eq!(tag.span.slice(input), format!("#{}", tag.name));
    }
    for section in &note.sections {
        assert_eq!(section.title_span.slice(input), section.title);
    }
    for task in &note.tasks {
        assert_eq!(task.text_span.slice(input), task.text);
    }
}
