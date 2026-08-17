//! Mind mapy — bloky `@mapa` s odsazeným seznamem uzlů.

use textura_core::mindmap::{render, suggest_id, NodeSpec};
use textura_core::parse;

/// Uzly k zápisu bez ID — v testech se ID uvádí jen tam, kde na něm záleží.
fn plain(nodes: &[(usize, &str)]) -> Vec<NodeSpec> {
    nodes
        .iter()
        .map(|(depth, text)| NodeSpec {
            depth: *depth,
            text: text.to_string(),
            color: None,
            id: None,
        })
        .collect()
}

#[test]
fn mapa_se_sklada_z_odsazeneho_seznamu() {
    let note = parse(
        "@mapa Infrastruktura\n\
         - Server\n  \
         - [[Docker]]\n    \
         - pět služeb\n  \
         - [[MariaDB]]\n",
    );

    assert_eq!(note.mindmaps.len(), 1);
    let map = &note.mindmaps[0];
    assert_eq!(map.title.as_deref(), Some("Infrastruktura"));
    assert_eq!(map.nodes.len(), 4);

    let depths: Vec<usize> = map.nodes.iter().map(|n| n.depth).collect();
    assert_eq!(depths, vec![0, 1, 2, 1]);

    let texts: Vec<&str> = map.nodes.iter().map(|n| n.text.as_str()).collect();
    assert_eq!(texts, vec!["Server", "[[Docker]]", "pět služeb", "[[MariaDB]]"]);

    assert_eq!(map.nodes[1].parent, Some(0));
    assert_eq!(map.nodes[2].parent, Some(1));
    assert_eq!(map.nodes[3].parent, Some(0));
    assert_eq!(map.nodes[0].children, vec![1, 3]);
    assert_eq!(map.roots().collect::<Vec<_>>(), vec![0]);
}

#[test]
fn odkaz_v_uzlu_je_bezny_odkaz() {
    // Kvůli tomu se mapa parsuje jako obyčejný text: `[[Docker]]` v uzlu se
    // musí počítat do backlinků stejně jako odkaz ve větě.
    let note = parse("@mapa\n- [[Docker|kontejnery]]\n");

    assert_eq!(note.links.len(), 1);
    assert_eq!(note.links[0].target, "Docker");
    assert_eq!(note.links[0].display_text(), "kontejnery");
    assert!(note.links[0].span.inside(&note.mindmaps[0].nodes[0].text_span));
}

#[test]
fn mapa_bez_nazvu_i_bez_uzlu() {
    let note = parse("Poznámka\n\n@mapa\n\ndalší text\n");

    assert_eq!(note.mindmaps.len(), 1);
    assert_eq!(note.mindmaps[0].title, None);
    assert!(note.mindmaps[0].nodes.is_empty());
    // Blok bez uzlů končí na konci vlastní hlavičky.
    assert_eq!(
        note.mindmaps[0].span,
        note.mindmaps[0].header_span,
    );
}

#[test]
fn blok_konci_prvnim_neseznamovym_radkem() {
    let note = parse("@mapa A\n- jeden\n\n- už jiný seznam\n");

    assert_eq!(note.mindmaps.len(), 1);
    assert_eq!(note.mindmaps[0].nodes.len(), 1);
    let text = "@mapa A\n- jeden\n\n- už jiný seznam\n";
    assert_eq!(note.mindmaps[0].span.slice(text), "@mapa A\n- jeden");
}

#[test]
fn v_poznamce_muze_byt_map_vic() {
    let note = parse("@mapa A\n- a\n\n@mindmap B\n- b\n  - c\n");

    assert_eq!(note.mindmaps.len(), 2);
    assert_eq!(note.mindmaps[0].title.as_deref(), Some("A"));
    assert_eq!(note.mindmaps[1].title.as_deref(), Some("B"));
    assert_eq!(note.mindmaps[1].nodes.len(), 2);
    assert_eq!(note.mindmaps[1].line, 4);
}

#[test]
fn mapou_neni_property_ani_bezny_text() {
    let note = parse("@title: Mapa\n@mapa: něco\n\nO mapách píšu rád.\n@mapaSveta\n- položka\n");

    assert!(note.mindmaps.is_empty());
}

#[test]
fn vic_korenu_je_les() {
    let note = parse("@mapa\n- první\n- druhý\n  - potomek\n");

    let map = &note.mindmaps[0];
    assert_eq!(map.roots().collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(map.nodes[2].parent, Some(1));
}

#[test]
fn odsazeni_tabulatorem_i_ctyrmi_mezerami() {
    let tabs = parse("@mapa\n- a\n\t- b\n\t\t- c\n");
    let spaces = parse("@mapa\n- a\n    - b\n        - c\n");

    let depths = |text: &str| -> Vec<usize> {
        parse(text).mindmaps[0].nodes.iter().map(|n| n.depth).collect()
    };
    assert_eq!(tabs.mindmaps[0].nodes.len(), 3);
    assert_eq!(spaces.mindmaps[0].nodes.len(), 3);
    assert_eq!(depths("@mapa\n- a\n\t- b\n\t\t- c\n"), vec![0, 1, 2]);
    assert_eq!(depths("@mapa\n- a\n    - b\n        - c\n"), vec![0, 1, 2]);
}

#[test]
fn odsazeny_prvni_uzel_je_stejne_koren() {
    // Ručně upravený soubor může mít celý blok odsazený.
    let note = parse("@mapa\n  - a\n    - b\n");

    let depths: Vec<usize> = note.mindmaps[0].nodes.iter().map(|n| n.depth).collect();
    assert_eq!(depths, vec![0, 1]);
}

#[test]
fn ukol_v_uzlu_zustane_ukolem() {
    let note = parse("@mapa\n- [ ] zařídit doménu\n");

    assert_eq!(note.mindmaps[0].nodes[0].text, "[ ] zařídit doménu");
    assert_eq!(note.tasks.len(), 1);
}

#[test]
fn spany_uzlu_miri_do_puvodniho_textu() {
    let text = "@mapa Žluťoučký\n- kůň [[Docker]]\n";
    let note = parse(text);

    let map = &note.mindmaps[0];
    assert_eq!(map.title_span.unwrap().slice(text), "Žluťoučký");
    assert_eq!(map.nodes[0].text_span.slice(text), "kůň [[Docker]]");
    assert_eq!(map.nodes[0].span.slice(text), "- kůň [[Docker]]");
}

#[test]
fn render_vyrobi_blok_ktery_se_zpatky_rozparsuje() {
    let nodes = plain(&[(0, "Server"), (1, "[[Docker]]"), (2, "pět služeb")]);
    let text = render(Some("Infrastruktura"), &nodes, &[], "\n");

    assert_eq!(
        text,
        "@mapa Infrastruktura\n- Server\n  - [[Docker]]\n    - pět služeb"
    );

    let note = parse(&format!("{text}\n"));
    let parsed: Vec<NodeSpec> = note.mindmaps[0]
        .nodes
        .iter()
        .map(|n| NodeSpec {
            depth: n.depth,
            text: n.text.clone(),
            color: n.color.clone(),
            id: n.id.clone(),
        })
        .collect();
    assert_eq!(parsed, nodes);
}

#[test]
fn render_srovna_prehnane_zanoreni() {
    // Kořen nemůže být odsazený a žádný uzel nepřeskočí víc než jednu úroveň —
    // jinak by se blok po znovunačtení rozpadl jinak, než ho UI poslalo.
    let text = render(
        None,
        &plain(&[(3, "kořen"), (9, "potomek"), (9, "vnuk")]),
        &[],
        "\n",
    );

    assert_eq!(text, "@mapa\n- kořen\n  - potomek\n  - vnuk");
}

#[test]
fn render_respektuje_crlf() {
    assert_eq!(render(None, &plain(&[(0, "a")]), &[], "\r\n"), "@mapa\r\n- a");
}

// --- stabilní ID a ruční spoje ---------------------------------------------

#[test]
fn uzel_muze_mit_stabilni_id() {
    let note = parse("@mapa\n- [[Docker]] ^docker\n  - bez id\n");

    let map = &note.mindmaps[0];
    assert_eq!(map.nodes[0].id.as_deref(), Some("docker"));
    assert_eq!(map.nodes[0].text, "[[Docker]]");
    assert_eq!(map.nodes[1].id, None);
    assert_eq!(map.node_by_id("DOCKER"), Some(0));
    // Odkaz se pořád parsuje, ID mu nesmí přebít rozsah.
    assert_eq!(note.links[0].target, "Docker");
    assert!(note.links[0].span.inside(&map.nodes[0].text_span));
}

#[test]
fn strizka_id_nespolkne_text_ktery_id_neni() {
    let note = parse("@mapa\n- 2^10 je 1024\n- ^samotne\n- text ^s mezerou\n");

    let ids: Vec<Option<String>> = note.mindmaps[0]
        .nodes
        .iter()
        .map(|n| n.id.clone())
        .collect();
    assert_eq!(ids, vec![None, None, None]);
    assert_eq!(note.mindmaps[0].nodes[1].text, "^samotne");
}

#[test]
fn spoje_propojuji_uzly_podle_id() {
    let note = parse(
        "@mapa Síť\n         - Server ^server\n           - [[Docker]] ^docker\n           - [[MariaDB]] ^db\n         @spoje\n         - docker -- db\n         - db -- server\n",
    );

    let map = &note.mindmaps[0];
    assert_eq!(map.edges.len(), 2);
    assert_eq!((map.edges[0].from_node, map.edges[0].to_node), (Some(1), Some(2)));
    assert_eq!((map.edges[1].from_node, map.edges[1].to_node), (Some(2), Some(0)));
}

#[test]
fn spoj_na_zmizely_uzel_zustane_v_textu_ale_nikam_nemiri() {
    // Soubor mohl někdo upravit ručně; nesmí to nic rozbít.
    let note = parse("@mapa\n- A ^a\n@spoje\n- a -- neexistuje\n");

    let edge = &note.mindmaps[0].edges[0];
    assert_eq!(edge.from_node, Some(0));
    assert_eq!(edge.to_node, None);
    assert_eq!(edge.to, "neexistuje");
}

#[test]
fn blok_spoju_patri_do_rozsahu_mapy() {
    let text = "@mapa\n- A ^a\n- B ^b\n@spoje\n- a -- b\n\nDalší text.\n";
    let note = parse(text);

    assert_eq!(
        note.mindmaps[0].span.slice(text),
        "@mapa\n- A ^a\n- B ^b\n@spoje\n- a -- b"
    );
}

#[test]
fn spoje_bez_mapy_se_neparsuji_jako_mapa() {
    let note = parse("@spoje\n- a -- b\n");

    assert!(note.mindmaps.is_empty());
}

#[test]
fn render_zapise_id_i_spoje_a_zpatky_se_precte() {
    let nodes = vec![
        NodeSpec { depth: 0, text: "Server".into(), color: None, id: Some("server".into()) },
        NodeSpec {
            depth: 1,
            text: "[[Docker]]".into(),
            color: None,
            id: Some("docker".into()),
        },
        NodeSpec { depth: 1, text: "bez id".into(), color: None, id: None },
    ];
    let edges = vec![("docker".to_string(), "server".to_string())];

    let text = render(Some("Síť"), &nodes, &edges, "\n");
    assert_eq!(
        text,
        "@mapa Síť\n- Server ^server\n  - [[Docker]] ^docker\n  - bez id\n@spoje\n- docker -- server"
    );

    let note = parse(&format!("{text}\n"));
    let map = &note.mindmaps[0];
    assert_eq!(map.nodes[1].id.as_deref(), Some("docker"));
    assert_eq!(map.edges[0].from_node, Some(1));
    assert_eq!(map.edges[0].to_node, Some(0));
}

#[test]
fn navrzene_id_vychazi_z_textu_uzlu() {
    assert_eq!(suggest_id("[[Docker]]", &[]), "docker");
    assert_eq!(suggest_id("Žluťoučký kůň", &[]), "zlutoucky-kun");
    assert_eq!(suggest_id("  ", &[]), "uzel");
    assert_eq!(suggest_id("[[MariaDB|databáze]]", &[]), "mariadb-databaze");
}

#[test]
fn navrzene_id_se_neopakuje() {
    let taken = vec!["docker".to_string(), "docker-2".to_string()];

    assert_eq!(suggest_id("Docker", &taken), "docker-3");
    // Porovnává se case-insensitive, ať v souboru nevzniknou dvě „stejná“ ID.
    assert_eq!(suggest_id("DOCKER", &vec!["Docker".to_string()]), "docker-2");
}

// --- barvy buněk -----------------------------------------------------------

#[test]
fn bunka_muze_mit_barvu() {
    let note = parse("@mapa\n- [[Docker]] @color:blue ^docker\n- bez barvy\n");

    let map = &note.mindmaps[0];
    assert_eq!(map.nodes[0].color.as_deref(), Some("blue"));
    assert_eq!(map.nodes[0].id.as_deref(), Some("docker"));
    assert_eq!(map.nodes[0].text, "[[Docker]]");
    assert_eq!(map.nodes[1].color, None);
    // Odkaz v obarvené buňce musí zůstat odkazem.
    assert_eq!(note.links[0].target, "Docker");
}

#[test]
fn barva_jde_i_bez_id() {
    let note = parse("@mapa\n- Server @color:green\n");

    assert_eq!(note.mindmaps[0].nodes[0].color.as_deref(), Some("green"));
    assert_eq!(note.mindmaps[0].nodes[0].text, "Server");
}

#[test]
fn color_uprostred_textu_barvou_neni() {
    let note = parse("@mapa\n- @color:blue je zápis barvy\n");

    assert_eq!(note.mindmaps[0].nodes[0].color, None);
    assert_eq!(note.mindmaps[0].nodes[0].text, "@color:blue je zápis barvy");
}

#[test]
fn render_zapise_barvu_pred_id() {
    let nodes = vec![NodeSpec {
        depth: 0,
        text: "[[Docker]]".into(),
        color: Some("blue".into()),
        id: Some("docker".into()),
    }];

    let text = render(None, &nodes, &[], "\n");
    assert_eq!(text, "@mapa\n- [[Docker]] @color:blue ^docker");

    let note = parse(&format!("{text}\n"));
    assert_eq!(note.mindmaps[0].nodes[0].color.as_deref(), Some("blue"));
    assert_eq!(note.mindmaps[0].nodes[0].id.as_deref(), Some("docker"));
}
