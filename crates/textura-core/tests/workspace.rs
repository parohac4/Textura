//! Testy indexu, resolveru odkazů, backlinků a fulltextu.

use std::fs;
use std::path::PathBuf;

use textura_core::workspace::{Resolution, Workspace};

/// Dočasný workspace, který se po testu sám uklidí.
struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let mut root = std::env::temp_dir();
        root.push(format!(
            "textura-test-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        for (path, content) in files {
            let full = root.join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, content).unwrap();
        }
        Self { root }
    }

    fn load(&self) -> Workspace {
        Workspace::scan(&self.root).unwrap()
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Přeloží první odkaz v dané poznámce.
fn resolve_first(ws: &Workspace, from_path: &str) -> Resolution {
    let from = ws.index_of_path(from_path).expect("zdrojová poznámka");
    let link = ws.note(from).unwrap().links.first().expect("odkaz");
    ws.resolve(link, Some(from))
}

fn path_of(ws: &Workspace, res: &Resolution) -> String {
    match res {
        Resolution::Note(i) => ws.note(*i).unwrap().path.clone(),
        other => panic!("čekal jsem Note, dostal {other:?}"),
    }
}

// --- scan ------------------------------------------------------------------

#[test]
fn scan_najde_poznamky_rekurzivne() {
    let tmp = TempWorkspace::new(
        "scan",
        &[
            ("Docker.txt", "Docker\n"),
            ("Projects/Server.txt", "Server\n"),
            ("Projects/Deep/Nested.txt", "Nested\n"),
        ],
    );
    let ws = tmp.load();
    assert_eq!(ws.notes().len(), 3);
    assert!(ws.index_of_path("Projects/Deep/Nested.txt").is_some());
}

#[test]
fn scan_ignoruje_skryte_slozky_a_neposkytuje_prilohy() {
    let tmp = TempWorkspace::new(
        "hidden",
        &[
            ("Docker.txt", "Docker\n"),
            (".txtnote/index.db", "binární smetí\n"),
            (".git/config", "[core]\n"),
            ("attachments/manual.pdf", "%PDF\n"),
        ],
    );
    let ws = tmp.load();
    let paths: Vec<_> = ws.notes().iter().map(|n| n.path.as_str()).collect();
    assert_eq!(paths, vec!["Docker.txt"]);
}

#[test]
fn index_zna_nazev_title_aliasy_i_tagy() {
    let tmp = TempWorkspace::new(
        "meta",
        &[(
            "Server.txt",
            "@title: Produkční server\n@aliases: Production, Prod\n\nText\n\n#server/linux\n",
        )],
    );
    let ws = tmp.load();
    let note = ws.note(ws.index_of_path("Server.txt").unwrap()).unwrap();
    assert_eq!(note.name, "Server");
    assert_eq!(note.title, "Produkční server");
    assert_eq!(note.aliases, vec!["Production", "Prod"]);
    assert_eq!(note.tags, vec!["server/linux"]);
    assert_eq!(note.link_path(), "Server");
}

#[test]
fn nazev_souboru_je_vychozi_titulek() {
    let tmp = TempWorkspace::new("default-title", &[("Docker.txt", "Docker\n")]);
    let ws = tmp.load();
    assert_eq!(ws.notes()[0].title, "Docker");
}

// --- složky ----------------------------------------------------------------

#[test]
fn scan_zna_i_prazdne_slozky() {
    let tmp = TempWorkspace::new(
        "folders",
        &[("Docker.txt", "a\n"), ("Projekty/Server.txt", "b\n")],
    );
    fs::create_dir_all(tmp.root.join("Prazdna/Hloubeji")).unwrap();

    let ws = tmp.load();
    assert_eq!(
        ws.folders(),
        vec!["Prazdna", "Prazdna/Hloubeji", "Projekty"]
    );
}

#[test]
fn skryte_slozky_se_do_seznamu_nedostanou() {
    let tmp = TempWorkspace::new("folders-hidden", &[("A.txt", "a\n")]);
    fs::create_dir_all(tmp.root.join(".txtnote")).unwrap();
    fs::create_dir_all(tmp.root.join(".git/objects")).unwrap();

    let ws = tmp.load();
    assert!(ws.folders().is_empty());
}

#[test]
fn vytvoreni_slozky_vcetne_nadrazenych() {
    let tmp = TempWorkspace::new("folder-create", &[("A.txt", "a\n")]);
    let mut ws = tmp.load();

    ws.create_folder("Projekty/2026/Q3").unwrap();

    assert!(tmp.root.join("Projekty/2026/Q3").is_dir());
    assert_eq!(ws.folders(), vec!["Projekty", "Projekty/2026", "Projekty/2026/Q3"]);
}

#[test]
fn vytvoreni_existujici_slozky_selze() {
    let tmp = TempWorkspace::new("folder-exists", &[("Projekty/A.txt", "a\n")]);
    let mut ws = tmp.load();
    assert!(ws.create_folder("Projekty").is_err());
}

#[test]
fn slozka_mimo_workspace_se_odmitne() {
    let tmp = TempWorkspace::new("folder-escape", &[("A.txt", "a\n")]);
    let mut ws = tmp.load();
    assert!(ws.create_folder("../zlo").is_err());
    assert!(ws.create_folder("/tmp/zlo").is_err());
    assert!(ws.create_folder("   ").is_err());
}

#[test]
fn zapis_poznamky_zaregistruje_novou_slozku() {
    let tmp = TempWorkspace::new("folder-from-note", &[("A.txt", "a\n")]);
    let mut ws = tmp.load();

    ws.write("Nova/Podslozka/B.txt", "b\n").unwrap();

    assert_eq!(ws.folders(), vec!["Nova", "Nova/Podslozka"]);
}

#[test]
fn prazdna_slozka_prezije_znovunacteni() {
    let tmp = TempWorkspace::new("folder-persist", &[("A.txt", "a\n")]);
    let mut ws = tmp.load();
    ws.create_folder("Prazdna").unwrap();

    let reloaded = tmp.load();
    assert_eq!(reloaded.folders(), vec!["Prazdna"]);
}

// --- bezpečnost cest -------------------------------------------------------

/// Vytvoří vedle workspace složku „mimo“ se souborem, který nikdo nemá číst
/// ani přepsat, a uvnitř workspace symlink, který na ni míří.
fn workspace_se_symlinkem(name: &str) -> (TempWorkspace, PathBuf) {
    use std::os::unix::fs::symlink;

    let tmp = TempWorkspace::new(name, &[("A.txt", "a\n")]);
    let outside = tmp.root.parent().unwrap().join(format!("{name}-mimo"));
    let _ = fs::remove_dir_all(&outside);
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("tajne.txt"), "TAJEMSTVÍ\n").unwrap();
    symlink(&outside, tmp.root.join("unik")).unwrap();
    (tmp, outside)
}

#[test]
fn symlink_ven_z_workspace_neprojde_pri_zapisu() {
    let (tmp, outside) = workspace_se_symlinkem("symlink-write");
    let mut ws = tmp.load();

    assert!(
        ws.write("unik/tajne.txt", "PŘEPSÁNO\n").is_err(),
        "zápis skrz symlink musí selhat"
    );
    assert_eq!(
        fs::read_to_string(outside.join("tajne.txt")).unwrap(),
        "TAJEMSTVÍ\n",
        "soubor mimo workspace zůstal nedotčený"
    );

    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn symlink_ven_z_workspace_neprojde_pri_cteni() {
    let (tmp, outside) = workspace_se_symlinkem("symlink-read");
    let ws = tmp.load();

    assert!(ws.read("unik/tajne.txt").is_err(), "čtení skrz symlink musí selhat");
    assert!(ws.absolute("unik/tajne.txt").is_none());

    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn symlink_ven_z_workspace_neprojde_ani_pri_vytvoreni() {
    // Kliknutí na [[unik/nova]] v cizí poznámce nesmí založit soubor venku.
    let (tmp, outside) = workspace_se_symlinkem("symlink-create");
    let mut ws = tmp.load();

    assert!(ws.create("unik/nova.txt", "obsah\n").is_err());
    assert!(!outside.join("nova.txt").exists());

    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn symlinky_se_neindexuji() {
    let (tmp, outside) = workspace_se_symlinkem("symlink-scan");
    fs::write(outside.join("cizi.txt"), "cizí poznámka\n").unwrap();
    // Symlink přímo na soubor, ne jen na složku.
    std::os::unix::fs::symlink(outside.join("cizi.txt"), tmp.root.join("Podvrh.txt")).unwrap();

    let ws = tmp.load();
    let paths: Vec<_> = ws.notes().iter().map(|n| n.path.as_str()).collect();
    assert_eq!(paths, vec!["A.txt"], "symlinky nepatří do indexu");
    assert!(ws.folders().is_empty(), "symlinkovaná složka nepatří do stromu");

    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn poznamka_musi_mit_priponu_txt() {
    // Odkaz [[skript.sh]] by jinak uživatele navedl k založení skriptu.
    let tmp = TempWorkspace::new("create-ext", &[("A.txt", "a\n")]);
    let mut ws = tmp.load();

    assert!(ws.create("skript.sh", "#!/bin/sh\nrm -rf /\n").is_err());
    assert!(ws.create(".bashrc", "evil\n").is_err());
    assert!(ws.create(".txt", "prazdny nazev\n").is_err());
    assert!(!tmp.root.join("skript.sh").exists());

    assert!(ws.create("Poznámka.TXT", "velká přípona projde\n").is_ok());
}

// --- resolver --------------------------------------------------------------

#[test]
fn jednoznacny_odkaz_v_celem_workspace() {
    let tmp = TempWorkspace::new(
        "unique",
        &[
            ("Notes/Index.txt", "[[Docker]]\n"),
            ("Servers/Docker.txt", "Docker\n"),
        ],
    );
    let ws = tmp.load();
    let res = resolve_first(&ws, "Notes/Index.txt");
    assert_eq!(path_of(&ws, &res), "Servers/Docker.txt");
}

#[test]
fn prednost_ma_poznamka_ve_stejne_slozce() {
    let tmp = TempWorkspace::new(
        "same-folder",
        &[
            ("Projects/Index.txt", "[[Docker]]\n"),
            ("Projects/Docker.txt", "Docker v projektech\n"),
            ("Servers/Docker.txt", "Docker na serverech\n"),
        ],
    );
    let ws = tmp.load();
    let res = resolve_first(&ws, "Projects/Index.txt");
    assert_eq!(path_of(&ws, &res), "Projects/Docker.txt");
}

#[test]
fn nejednoznacny_odkaz_nabidne_vyber() {
    let tmp = TempWorkspace::new(
        "ambiguous",
        &[
            ("Index.txt", "[[Docker]]\n"),
            ("Projects/Docker.txt", "a\n"),
            ("Servers/Docker.txt", "b\n"),
        ],
    );
    let ws = tmp.load();
    match resolve_first(&ws, "Index.txt") {
        Resolution::Ambiguous(candidates) => {
            let mut paths: Vec<_> = candidates
                .iter()
                .map(|i| ws.note(*i).unwrap().path.clone())
                .collect();
            paths.sort();
            assert_eq!(paths, vec!["Projects/Docker.txt", "Servers/Docker.txt"]);
        }
        other => panic!("čekal jsem Ambiguous, dostal {other:?}"),
    }
}

#[test]
fn explicitni_cesta_rozhodne_i_pri_konfliktu() {
    let tmp = TempWorkspace::new(
        "explicit",
        &[
            ("Index.txt", "[[Projects/Docker]]\n"),
            ("Projects/Docker.txt", "a\n"),
            ("Servers/Docker.txt", "b\n"),
        ],
    );
    let ws = tmp.load();
    let res = resolve_first(&ws, "Index.txt");
    assert_eq!(path_of(&ws, &res), "Projects/Docker.txt");
}

#[test]
fn odkaz_pres_alias() {
    let tmp = TempWorkspace::new(
        "alias",
        &[
            ("Index.txt", "[[Docker Engine]]\n"),
            ("Docker.txt", "@aliases: Docker Engine, Containers\n\nDocker\n"),
        ],
    );
    let ws = tmp.load();
    let res = resolve_first(&ws, "Index.txt");
    assert_eq!(path_of(&ws, &res), "Docker.txt");
}

#[test]
fn stejny_alias_u_vic_poznamek_nabidne_vyber() {
    let tmp = TempWorkspace::new(
        "alias-conflict",
        &[
            ("Index.txt", "[[Prod]]\n"),
            ("A.txt", "@aliases: Prod\n"),
            ("B.txt", "@aliases: Prod\n"),
        ],
    );
    let ws = tmp.load();
    assert!(matches!(
        resolve_first(&ws, "Index.txt"),
        Resolution::Ambiguous(_)
    ));
}

#[test]
fn nazev_souboru_ma_prednost_pred_aliasem() {
    let tmp = TempWorkspace::new(
        "name-beats-alias",
        &[
            ("Index.txt", "[[Docker]]\n"),
            ("Docker.txt", "Skutečný Docker\n"),
            ("Jiná.txt", "@aliases: Docker\n"),
        ],
    );
    let ws = tmp.load();
    let res = resolve_first(&ws, "Index.txt");
    assert_eq!(path_of(&ws, &res), "Docker.txt");
}

#[test]
fn neexistujici_odkaz_navrhne_cestu_k_vytvoreni() {
    let tmp = TempWorkspace::new(
        "unresolved",
        &[("Projects/Index.txt", "[[Kubernetes]]\n")],
    );
    let ws = tmp.load();
    assert_eq!(
        resolve_first(&ws, "Projects/Index.txt"),
        Resolution::Unresolved {
            suggested_path: "Projects/Kubernetes.txt".into()
        }
    );
}

#[test]
fn odkaz_na_existujici_prilohu() {
    let tmp = TempWorkspace::new(
        "attachment",
        &[
            ("Index.txt", "[[attachments/manual.pdf]]\n"),
            ("attachments/manual.pdf", "%PDF\n"),
        ],
    );
    let ws = tmp.load();
    assert_eq!(
        resolve_first(&ws, "Index.txt"),
        Resolution::Attachment("attachments/manual.pdf".into())
    );
}

#[test]
fn odkaz_na_chybejici_prilohu_je_unresolved() {
    let tmp = TempWorkspace::new("missing-attachment", &[("Index.txt", "[[attachments/chybi.pdf]]\n")]);
    let ws = tmp.load();
    assert!(matches!(
        resolve_first(&ws, "Index.txt"),
        Resolution::Unresolved { .. }
    ));
}

#[test]
fn odkaz_na_vlastni_sekci_zustava_v_poznamce() {
    let tmp = TempWorkspace::new("self", &[("Docker.txt", "[[#Instalace]]\n\n== Instalace ==\n")]);
    let ws = tmp.load();
    let i = ws.index_of_path("Docker.txt").unwrap();
    assert_eq!(resolve_first(&ws, "Docker.txt"), Resolution::Note(i));
}

#[test]
fn cesta_mimo_workspace_se_odmitne() {
    let tmp = TempWorkspace::new("escape", &[("Docker.txt", "Docker\n")]);
    let ws = tmp.load();
    assert!(ws.absolute("../tajne.txt").is_none());
    assert!(ws.absolute("/etc/passwd").is_none());
    assert!(ws.absolute("Docker.txt").is_some());
}

#[test]
fn unresolved_links_najde_chybejici_poznamky() {
    let tmp = TempWorkspace::new(
        "missing",
        &[
            ("Index.txt", "[[Docker]]\n[[Kubernetes]]\n"),
            ("Docker.txt", "Docker\n"),
        ],
    );
    let ws = tmp.load();
    let missing: Vec<_> = ws
        .unresolved_links()
        .iter()
        .map(|(_, l)| l.target.clone())
        .collect();
    assert_eq!(missing, vec!["Kubernetes"]);
}

// --- backlinky -------------------------------------------------------------

#[test]
fn backlinky_najdou_odkazujici_poznamky() {
    let tmp = TempWorkspace::new(
        "backlinks",
        &[
            ("Docker.txt", "Docker\n"),
            ("Server.txt", "Deployment používá:\n[[Docker]]\n"),
            ("Mautic.txt", "Běží v [[Docker|kontejneru]].\n"),
            ("Nesouvisi.txt", "Nic tu není.\n"),
        ],
    );
    let ws = tmp.load();
    let docker = ws.index_of_path("Docker.txt").unwrap();
    let backlinks = ws.backlinks(docker);

    let mut from: Vec<_> = backlinks
        .iter()
        .map(|b| ws.note(b.from).unwrap().path.clone())
        .collect();
    from.sort();
    assert_eq!(from, vec!["Mautic.txt", "Server.txt"]);

    let mautic = backlinks
        .iter()
        .find(|b| ws.note(b.from).unwrap().path == "Mautic.txt")
        .unwrap();
    assert_eq!(mautic.context, "Běží v [[Docker|kontejneru]].");
    assert_eq!(mautic.line, 1);
}

#[test]
fn backlinky_respektuji_resolver() {
    // [[Docker]] z Projects musí vést na Projects/Docker.txt, takže
    // Servers/Docker.txt žádný backlink nedostane.
    let tmp = TempWorkspace::new(
        "backlinks-resolver",
        &[
            ("Projects/Index.txt", "[[Docker]]\n"),
            ("Projects/Docker.txt", "a\n"),
            ("Servers/Docker.txt", "b\n"),
        ],
    );
    let ws = tmp.load();
    let projects = ws.index_of_path("Projects/Docker.txt").unwrap();
    let servers = ws.index_of_path("Servers/Docker.txt").unwrap();
    assert_eq!(ws.backlinks(projects).len(), 1);
    assert_eq!(ws.backlinks(servers).len(), 0);
}

#[test]
fn poznamka_neodkazuje_sama_na_sebe() {
    let tmp = TempWorkspace::new("self-link", &[("Docker.txt", "[[Docker]]\n")]);
    let ws = tmp.load();
    let docker = ws.index_of_path("Docker.txt").unwrap();
    assert!(ws.backlinks(docker).is_empty());
}

// --- hledání ---------------------------------------------------------------

#[test]
fn fulltext_je_case_insensitive() {
    let tmp = TempWorkspace::new(
        "search",
        &[
            ("Docker.txt", "Docker používám pro provoz služeb.\n"),
            ("Server.txt", "Server běží na Debianu.\n"),
        ],
    );
    let ws = tmp.load();
    let hits = ws.search("DOCKER", 50);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "Docker.txt");
    assert_eq!(hits[0].line, 1);
    assert_eq!(hits[0].snippet, "Docker používám pro provoz služeb.");
}

#[test]
fn fulltext_vraci_platny_span_i_s_diakritikou() {
    let tmp = TempWorkspace::new(
        "search-span",
        &[("A.txt", "První řádek\nPříliš žluťoučký kůň\n")],
    );
    let ws = tmp.load();
    let hits = ws.search("žluťoučký", 50);
    assert_eq!(hits.len(), 1);
    let content = &ws.note(ws.index_of_path("A.txt").unwrap()).unwrap().content;
    assert_eq!(hits[0].span.slice(content), "žluťoučký");
}

#[test]
fn fulltext_drzi_span_i_kdyz_lowercase_meni_pocet_znaku() {
    // Turecké İ se v lowercase rozpadne na dva znaky (i + kombinující tečka).
    // Přepočet přes počty znaků by rozsah nálezu posunul.
    let tmp = TempWorkspace::new("search-lowercase", &[("A.txt", "İstanbul a potom docker\n")]);
    let ws = tmp.load();

    let hits = ws.search("docker", 10);
    assert_eq!(hits.len(), 1);
    let content = &ws.note(ws.index_of_path("A.txt").unwrap()).unwrap().content;
    assert_eq!(hits[0].span.slice(content), "docker");
}

#[test]
fn fulltext_respektuje_limit() {
    let tmp = TempWorkspace::new("search-limit", &[("A.txt", "x\nx\nx\nx\nx\n")]);
    let ws = tmp.load();
    assert_eq!(ws.search("x", 3).len(), 3);
}

#[test]
fn prazdny_dotaz_nevraci_nic() {
    let tmp = TempWorkspace::new("search-empty", &[("A.txt", "text\n")]);
    let ws = tmp.load();
    assert!(ws.search("   ", 50).is_empty());
}

// --- tagy ------------------------------------------------------------------

#[test]
fn hledani_podle_tagu_zahrnuje_potomky() {
    let tmp = TempWorkspace::new(
        "tags",
        &[
            ("A.txt", "#server\n"),
            ("B.txt", "#server/linux\n"),
            ("C.txt", "#projekt\n"),
        ],
    );
    let ws = tmp.load();
    let mut paths: Vec<_> = ws
        .notes_with_tag("server")
        .iter()
        .map(|i| ws.note(*i).unwrap().path.clone())
        .collect();
    paths.sort();
    assert_eq!(paths, vec!["A.txt", "B.txt"]);
    assert_eq!(ws.notes_with_tag("#projekt").len(), 1);
}

#[test]
fn prehled_tagu_s_pocty() {
    let tmp = TempWorkspace::new(
        "tag-counts",
        &[
            ("A.txt", "#server\n#linux\n"),
            ("B.txt", "#server\n#server\n"),
        ],
    );
    let ws = tmp.load();
    let counts = ws.tag_counts();
    // #server je ve dvou poznámkách, v jedné dvakrát — počítáme poznámky.
    assert_eq!(counts, vec![("linux".to_string(), 1), ("server".to_string(), 2)]);
}

// --- zápis -----------------------------------------------------------------

#[test]
fn zapis_aktualizuje_index_a_zachova_stejny_index() {
    let tmp = TempWorkspace::new(
        "write",
        &[("Docker.txt", "Docker\n"), ("Server.txt", "Server\n")],
    );
    let mut ws = tmp.load();
    let before = ws.index_of_path("Docker.txt").unwrap();

    let after = ws
        .write("Docker.txt", "@title: Docker Engine\n\n[[Server]]\n")
        .unwrap();

    assert_eq!(before, after, "index otevřené poznámky se nesmí posunout");
    assert_eq!(ws.note(after).unwrap().title, "Docker Engine");
    assert_eq!(ws.note(after).unwrap().links.len(), 1);

    let server = ws.index_of_path("Server.txt").unwrap();
    assert_eq!(ws.backlinks(server).len(), 1);
}

#[test]
fn vytvoreni_poznamky_z_unresolved_odkazu() {
    let tmp = TempWorkspace::new("create", &[("Index.txt", "[[Kubernetes]]\n")]);
    let mut ws = tmp.load();

    let Resolution::Unresolved { suggested_path } = resolve_first(&ws, "Index.txt") else {
        panic!("čekal jsem Unresolved");
    };
    let created = ws.create(&suggested_path, "Kubernetes\n").unwrap();
    assert_eq!(ws.note(created).unwrap().path, "Kubernetes.txt");

    // Odkaz teď vede na novou poznámku.
    assert_eq!(resolve_first(&ws, "Index.txt"), Resolution::Note(created));
}

#[test]
fn vytvoreni_existujici_poznamky_selze() {
    let tmp = TempWorkspace::new("create-exists", &[("Docker.txt", "Docker\n")]);
    let mut ws = tmp.load();
    assert!(ws.create("Docker.txt", "nový obsah\n").is_err());
    // Původní obsah zůstal nedotčený.
    assert_eq!(ws.read("Docker.txt").unwrap(), "Docker\n");
}

#[test]
fn zapis_mimo_workspace_selze() {
    let tmp = TempWorkspace::new("write-escape", &[("Docker.txt", "Docker\n")]);
    let mut ws = tmp.load();
    assert!(ws.write("../unik.txt", "zlo\n").is_err());
}

#[test]
fn read_vraci_aktualni_obsah_z_disku() {
    let tmp = TempWorkspace::new("read", &[("Docker.txt", "Docker\n")]);
    let ws = tmp.load();
    // Externí úprava mimo aplikaci.
    fs::write(tmp.root.join("Docker.txt"), "Upraveno ve Vimu\n").unwrap();
    assert_eq!(ws.read("Docker.txt").unwrap(), "Upraveno ve Vimu\n");
}
