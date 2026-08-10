//! Workspace: složka s `.txt` poznámkami, jejich index, resolver odkazů,
//! backlinky a fulltext.
//!
//! Index je pouze cache. Celý se dá kdykoli postavit znovu jen z obsahu složky
//! — proto tu není žádný perzistentní stav, jen `scan()`.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::links::{LinkKind, WikiLink};
use crate::search::{fold, fold_with_offsets, Query};
use crate::text::Span;
use crate::{parse, Note};

/// Přípona poznámky.
pub const NOTE_EXT: &str = "txt";
/// Složka s technickými metadaty aplikace.
pub const DATA_DIR: &str = ".txtnote";
/// Koš uvnitř `.txtnote/`. Smazané poznámky se sem přesouvají, aby šlo
/// překliknutí vzít zpět — soubory tam zůstávají čitelné jako obyčejný text.
pub const TRASH_DIR: &str = "trash";
/// Barvy složek. Složka nemá vlastní soubor, kam by se daly zapsat, takže
/// jako jediné jsou uložené mimo poznámky.
pub const COLORS_FILE: &str = "colors.txt";
/// Property, ve které si poznámka nese svoji barvu.
pub const COLOR_PROPERTY: &str = "color";

/// Povolené barvy. Ukládá se název, ne konkrétní odstín — ten si dosadí UI
/// podle motivu, takže poznámka zůstane čitelná i po přepnutí na tmavý.
pub const COLORS: [&str; 8] = [
    "red", "orange", "yellow", "green", "teal", "blue", "purple", "pink",
];

/// Ověří, že barva patří do palety.
fn check_color(color: Option<&str>) -> io::Result<Option<String>> {
    match color {
        None => Ok(None),
        Some(c) => {
            let c = c.trim().to_lowercase();
            if COLORS.contains(&c.as_str()) {
                Ok(Some(c))
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("neznámá barva: {c}"),
                ))
            }
        }
    }
}

/// Co se stalo s odkazy při přesunu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MoveReport {
    /// Kolik odkazů s explicitní cestou se přepsalo.
    pub updated_links: usize,
    /// V kolika poznámkách.
    pub updated_notes: usize,
    /// Odkazy holým názvem, které na přesouvanou poznámku mířily. Ty se
    /// nepřepisují — pokud zůstal název stejný, dál fungují; při změně názvu
    /// se z nich stanou nerozřešené odkazy.
    pub bare_links: usize,
}

/// Jedna poznámka v indexu.
#[derive(Debug, Clone)]
pub struct IndexedNote {
    /// Cesta relativní ke kořeni workspace, s `/` a včetně `.txt`.
    pub path: String,
    /// Název souboru bez `.txt` — výchozí název poznámky.
    pub name: String,
    /// Složka poznámky relativně ke kořeni; `""` pro kořen.
    pub folder: String,
    /// `@title`, jinak `name`.
    pub title: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub links: Vec<WikiLink>,
    pub content: String,
    /// Barva z `@color`, pokud je nastavená a patří do palety.
    pub color: Option<String>,
}

impl IndexedNote {
    /// Cesta bez přípony — tvar, kterým na poznámku míří `[[odkaz]]`.
    pub fn link_path(&self) -> &str {
        self.path
            .strip_suffix(&format!(".{NOTE_EXT}"))
            .unwrap_or(&self.path)
    }
}

/// Výsledek překladu wiki odkazu na cíl.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Odkaz míří na jednu konkrétní poznámku (index do `Workspace::notes`).
    Note(usize),
    /// Odkaz je nejednoznačný — aplikace musí nabídnout výběr.
    Ambiguous(Vec<usize>),
    /// Cílem je příloha; hodnota je cesta relativní ke kořeni.
    Attachment(String),
    /// Cíl neexistuje. Odkaz je platný jako unresolved link a kliknutí
    /// nabídne vytvoření poznámky na této cestě (včetně `.txt`).
    Unresolved { suggested_path: String },
}

/// Kde se nález potkal s dotazem. Určuje i pořadí výsledků — poznámka, která
/// se jmenuje jako dotaz, je skoro vždycky ta hledaná.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HitKind {
    /// Název poznámky (`@title`, jinak název souboru).
    Title,
    /// Jeden z `@aliases`.
    Alias,
    /// Tělo poznámky.
    Body,
}

/// Nález fulltextu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub path: String,
    pub kind: HitKind,
    /// Řádek nálezu (1-based). U nálezu v názvu nebo aliasu je to řádek
    /// příslušné property, a pokud poznámka název nese jen jménem souboru, `1`.
    pub line: usize,
    /// Text k zobrazení: řádek s nálezem ořezaný o okolní whitespace,
    /// u názvu a aliasu jejich vlastní text.
    pub snippet: String,
    /// Rozsah nálezu v obsahu poznámky. Prázdný, pokud nález v obsahu není —
    /// například název odvozený od jména souboru.
    pub span: Span,
    /// Rozsah nálezu uvnitř `snippet`, pro zvýraznění v seznamu výsledků.
    pub snippet_span: Span,
}

/// Zpětný odkaz na poznámku.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlink {
    /// Poznámka, ze které odkaz vede.
    pub from: usize,
    pub line: usize,
    /// Text řádku s odkazem.
    pub context: String,
}

#[derive(Debug, Clone, Default)]
pub struct Workspace {
    root: PathBuf,
    notes: Vec<IndexedNote>,
    /// Všechny složky workspace, seřazené. Sledují se zvlášť od poznámek,
    /// protože prázdná složka se z cest poznámek odvodit nedá.
    folders: BTreeSet<String>,
    /// Barvy složek načtené z `.txtnote/colors.txt`.
    folder_colors: std::collections::BTreeMap<String, String>,
    by_path: HashMap<String, usize>,
    by_name: HashMap<String, Vec<usize>>,
    by_alias: HashMap<String, Vec<usize>>,
}

fn lower(s: &str) -> String {
    s.to_lowercase()
}

/// Odmítne cesty, které by vedly mimo workspace (`..`, absolutní cesty).
fn is_safe_relative(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    let p = Path::new(path);
    p.is_relative()
        && p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

impl Workspace {
    /// Načte workspace ze složky. Prochází se rekurzivně, skryté složky
    /// (`.txtnote`, `.git`, …) se přeskakují.
    pub fn scan(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        let mut paths = Vec::new();
        let mut folders = BTreeSet::new();
        collect_notes(&root, &root, &mut paths, &mut folders)?;
        paths.sort();

        let mut ws = Workspace {
            root,
            folders,
            ..Default::default()
        };
        ws.folder_colors = ws.load_folder_colors();

        for rel in paths {
            let abs = ws.root.join(&rel);
            let Ok(bytes) = fs::read(&abs) else {
                continue; // soubor mohl mezitím zmizet — index je jen cache
            };
            let content = String::from_utf8_lossy(&bytes).into_owned();
            ws.insert(rel, content);
        }

        ws.rebuild_lookups();
        Ok(ws)
    }

    fn insert(&mut self, rel_path: String, content: String) {
        let indexed = build_indexed(rel_path, content);
        self.notes.push(indexed);
    }
    fn rebuild_lookups(&mut self) {
        self.by_path.clear();
        self.by_name.clear();
        self.by_alias.clear();

        for (i, note) in self.notes.iter().enumerate() {
            self.by_path.insert(lower(&note.path), i);
            self.by_name.entry(lower(&note.name)).or_default().push(i);
            // Na poznámku se dá odkázat i jejím @title.
            if !note.title.eq_ignore_ascii_case(&note.name) {
                self.by_alias.entry(lower(&note.title)).or_default().push(i);
            }
            for alias in &note.aliases {
                self.by_alias.entry(lower(alias)).or_default().push(i);
            }
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn notes(&self) -> &[IndexedNote] {
        &self.notes
    }

    /// Všechny složky workspace relativně ke kořeni, včetně prázdných.
    pub fn folders(&self) -> Vec<&str> {
        self.folders.iter().map(String::as_str).collect()
    }

    /// Zaregistruje složku i všechny její nadřazené složky.
    fn register_folder(&mut self, folder: &str) {
        let mut path = String::new();
        for segment in folder.split('/').filter(|s| !s.is_empty()) {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(segment);
            self.folders.insert(path.clone());
        }
    }

    /// Vytvoří složku (včetně chybějících nadřazených).
    ///
    /// Prázdná složka je legitimní stav — uživatel si může připravit strukturu
    /// dřív, než do ní něco napíše.
    pub fn create_folder(&mut self, rel_path: &str) -> io::Result<()> {
        // Ořezat se smí jen konec: `trim_matches('/')` by z absolutní cesty
        // `/tmp/zlo` udělal relativní `tmp/zlo` a obešel kontrolu níž.
        let normalized = rel_path.trim().trim_end_matches('/').to_string();
        if !is_safe_relative(&normalized) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "neplatný název složky",
            ));
        }
        let abs = self
            .absolute(&normalized)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cesta mimo workspace"))?;
        if abs.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "složka už existuje",
            ));
        }
        fs::create_dir_all(&abs)?;
        self.register_folder(&normalized);
        Ok(())
    }

    pub fn note(&self, index: usize) -> Option<&IndexedNote> {
        self.notes.get(index)
    }

    /// Poznámka podle cesty relativní ke kořeni (case-insensitive).
    pub fn index_of_path(&self, path: &str) -> Option<usize> {
        self.by_path.get(&lower(path)).copied()
    }

    /// Absolutní cesta k souboru ve workspace. `None`, pokud by cesta vedla ven.
    ///
    /// Textová kontrola (`..`, absolutní cesty) nestačí — symlink umístěný
    /// uvnitř workspace by jinak dovolil číst i přepisovat soubory kdekoli
    /// jinde. Cesta se proto ještě rozbalí a ověří proti skutečnému kořeni.
    pub fn absolute(&self, rel_path: &str) -> Option<PathBuf> {
        if !is_safe_relative(rel_path) {
            return None;
        }
        let joined = self.root.join(rel_path);
        let root = self.root.canonicalize().ok()?;

        // U dosud neexistujícího souboru ověřujeme nejbližšího existujícího
        // předka — ten už symlinkem být může.
        let mut probe = joined.clone();
        let real = loop {
            match probe.canonicalize() {
                Ok(resolved) => break resolved,
                Err(_) if probe.pop() => continue,
                Err(_) => return None,
            }
        };

        real.starts_with(&root).then_some(joined)
    }

    /// Přeloží wiki odkaz podle pravidel ze specifikace:
    ///
    /// 1. poznámka ve stejné složce,
    /// 2. jednoznačná poznámka v celém workspace,
    /// 3. alias (nebo `@title`),
    /// 4. víc kandidátů → výběr.
    ///
    /// `from` je index poznámky, ve které odkaz stojí.
    pub fn resolve(&self, link: &WikiLink, from: Option<usize>) -> Resolution {
        if link.kind == LinkKind::Attachment {
            let path = link.target.clone();
            return match self.absolute(&path) {
                Some(abs) if abs.exists() => Resolution::Attachment(path),
                _ => Resolution::Unresolved {
                    suggested_path: path,
                },
            };
        }

        let target = link.target.trim();
        if target.is_empty() {
            // Odkaz jen na vlastní sekci zůstává v aktuální poznámce.
            return match from {
                Some(i) => Resolution::Note(i),
                None => Resolution::Unresolved {
                    suggested_path: String::new(),
                },
            };
        }

        // Explicitní cesta má přednost před vším ostatním.
        if target.contains('/') {
            let path = format!("{target}.{NOTE_EXT}");
            if let Some(i) = self.index_of_path(&path) {
                return Resolution::Note(i);
            }
            return Resolution::Unresolved {
                suggested_path: path,
            };
        }

        let key = lower(target);
        let by_name = self.by_name.get(&key).cloned().unwrap_or_default();

        // 1. stejná složka
        if let Some(folder) = from.and_then(|i| self.notes.get(i)).map(|n| &n.folder) {
            if let Some(i) = by_name
                .iter()
                .find(|i| &self.notes[**i].folder == folder)
                .copied()
            {
                return Resolution::Note(i);
            }
        }

        // 2. jednoznačná poznámka v celém workspace
        match by_name.len() {
            1 => return Resolution::Note(by_name[0]),
            0 => {}
            _ => return Resolution::Ambiguous(by_name),
        }

        // 3. alias
        if let Some(by_alias) = self.by_alias.get(&key) {
            match by_alias.len() {
                1 => return Resolution::Note(by_alias[0]),
                0 => {}
                // 4. víc poznámek se stejným aliasem → výběr
                _ => return Resolution::Ambiguous(by_alias.clone()),
            }
        }

        // Poznámka zatím neexistuje — nabídneme ji vytvořit vedle zdroje.
        let folder = from
            .and_then(|i| self.notes.get(i))
            .map(|n| n.folder.clone())
            .unwrap_or_default();
        let suggested_path = if folder.is_empty() {
            format!("{target}.{NOTE_EXT}")
        } else {
            format!("{folder}/{target}.{NOTE_EXT}")
        };
        Resolution::Unresolved { suggested_path }
    }

    /// Zpětné odkazy na poznámku `to`.
    pub fn backlinks(&self, to: usize) -> Vec<Backlink> {
        let mut out = Vec::new();
        for (from, note) in self.notes.iter().enumerate() {
            if from == to {
                continue;
            }
            for link in &note.links {
                if link.kind != LinkKind::Note || link.is_internal() {
                    continue;
                }
                if self.resolve(link, Some(from)) != Resolution::Note(to) {
                    continue;
                }
                let context = note
                    .content
                    .lines()
                    .nth(link.line.saturating_sub(1))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                out.push(Backlink {
                    from,
                    line: link.line,
                    context,
                });
            }
        }
        out
    }

    /// Odkazy, které zatím nikam nevedou — vstup pro „chybějící poznámky“.
    pub fn unresolved_links(&self) -> Vec<(usize, &WikiLink)> {
        let mut out = Vec::new();
        for (i, note) in self.notes.iter().enumerate() {
            for link in &note.links {
                if matches!(self.resolve(link, Some(i)), Resolution::Unresolved { .. }) {
                    out.push((i, link));
                }
            }
        }
        out
    }

    /// Fulltext přes názvy, aliasy a obsah poznámek.
    ///
    /// Porovnává se bez ohledu na velikost písmen a na diakritiku (`zluty`
    /// najde `Žluťoučký`) a výrazy oddělené mezerami platí jako AND — poznámka
    /// musí obsahovat všechny, ale je jedno kde a v jakém pořadí. Vypíšou se
    /// pak ty řádky, na kterých je aspoň jeden z výrazů, takže je vidět
    /// kontext obou stran dotazu.
    ///
    /// Nálezy v názvu a v aliasu jdou první — kdo hledá `docker`, chce nejspíš
    /// poznámku *Docker*, ne dvacet zmínek o ní.
    ///
    /// `limit` omezuje počet nálezů, aby UI nedostalo desetitisíce řádků.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let query = Query::parse(query);
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }

        let mut named: Vec<SearchHit> = Vec::new();
        let mut body: Vec<SearchHit> = Vec::new();

        for note in &self.notes {
            // Název a aliasy jsou samostatný nález — poznámka *Docker* se má
            // najít i tehdy, když se v jejím těle slovo „docker“ neobjeví.
            let hit = if query.matches(&note.title) {
                Some(self.named_hit(note, &query, HitKind::Title, &note.title))
            } else {
                note.aliases
                    .iter()
                    .find(|a| query.matches(a))
                    .map(|alias| self.named_hit(note, &query, HitKind::Alias, alias))
            };

            // Řádek property, ze které nález v názvu pochází, se v těle
            // přeskočí — jinak by se `@title: Docker` vypsal dvakrát. Když se
            // property nenašla (název je jen jméno souboru), přeskakovat není
            // co a řádek zůstává normálním kandidátem.
            let skip_line = hit
                .as_ref()
                .filter(|h| !h.span.is_empty())
                .map(|h| h.line);
            named.extend(hit);

            if body.len() >= limit || !query.matches_folded(&fold(&note.content)) {
                continue;
            }

            let mut offset = 0usize;
            for (n, line) in note.content.split_inclusive('\n').enumerate() {
                if Some(n + 1) == skip_line {
                    offset += line.len();
                    continue;
                }
                // Skládání mění délku, takže offsety nálezu hledáme přes mapu
                // zpátky do původního řádku — tím zůstanou spany platné.
                let (folded, map) = fold_with_offsets(line);
                if let Some((rel_start, rel_end)) = query.first_hit_folded(&folded) {
                    let start = map[rel_start];
                    let end = map[rel_end];
                    let trimmed = line.trim();
                    let lead = line.len() - line.trim_start().len();
                    body.push(SearchHit {
                        path: note.path.clone(),
                        kind: HitKind::Body,
                        line: n + 1,
                        snippet: trimmed.to_string(),
                        span: Span::new(offset + start, offset + end),
                        snippet_span: Span::new(
                            start.saturating_sub(lead),
                            end.saturating_sub(lead),
                        ),
                    });
                    if body.len() >= limit {
                        break;
                    }
                }
                offset += line.len();
            }
        }

        named.extend(body);
        named.truncate(limit);
        named
    }

    /// Nález v názvu nebo aliasu poznámky.
    ///
    /// Řádek a rozsah v obsahu se dohledají v property, ze které text pochází.
    /// Když poznámka název nese jen jménem souboru, v obsahu žádný nález není
    /// a zbyde prázdný rozsah na prvním řádku.
    fn named_hit(&self, note: &IndexedNote, query: &Query, kind: HitKind, text: &str) -> SearchHit {
        let (folded, map) = fold_with_offsets(text);
        // Nález tu být musí — text prošel `matches` —, ale spadnout kvůli
        // zvýraznění by bylo hloupé, takže se v krajním případě označí celý.
        let (rel_start, rel_end) = query.first_hit_folded(&folded).unwrap_or((0, folded.len()));
        let snippet_span = Span::new(map[rel_start], map[rel_end]);

        let key = match kind {
            HitKind::Title => "title",
            HitKind::Alias => "aliases",
            HitKind::Body => "",
        };
        let (line, span) = parse(&note.content)
            .property(key)
            .and_then(|p| {
                let value = p.value_span.slice(&note.content);
                let at = p.value_span.start + value.find(text)?;
                Some((
                    p.line,
                    Span::new(at + snippet_span.start, at + snippet_span.end),
                ))
            })
            .unwrap_or((1, Span::new(0, 0)));

        SearchHit {
            path: note.path.clone(),
            kind,
            line,
            snippet: text.to_string(),
            span,
            snippet_span,
        }
    }

    /// Poznámky s daným tagem, včetně hierarchických potomků
    /// (`#server` najde i `#server/linux`).
    pub fn notes_with_tag(&self, tag: &str) -> Vec<usize> {
        let key = lower(tag.trim_start_matches('#'));
        self.notes
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                n.tags
                    .iter()
                    .any(|t| {
                        let t = lower(t);
                        t == key || t.starts_with(&format!("{key}/"))
                    })
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Všechny tagy ve workspace s počtem výskytů, seřazené podle názvu.
    pub fn tag_counts(&self) -> Vec<(String, usize)> {
        let mut counts: HashMap<String, (String, usize)> = HashMap::new();
        for note in &self.notes {
            let mut seen: Vec<String> = Vec::new();
            for tag in &note.tags {
                let key = lower(tag);
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key.clone());
                let entry = counts.entry(key).or_insert((tag.clone(), 0));
                entry.1 += 1;
            }
        }
        let mut out: Vec<(String, usize)> = counts.into_values().collect();
        out.sort_by(|a, b| lower(&a.0).cmp(&lower(&b.0)));
        out
    }

    /// Přečte poznámku z disku (ne z indexu) — vždy aktuální obsah.
    pub fn read(&self, rel_path: &str) -> io::Result<String> {
        let abs = self
            .absolute(rel_path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cesta mimo workspace"))?;
        let bytes = fs::read(abs)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Uloží poznámku a aktualizuje index.
    ///
    /// Konce řádků se nechávají tak, jak je poslal editor — o zachování
    /// původního stylu se stará vrstva nad tímto (viz [`Note::line_ending`]).
    pub fn write(&mut self, rel_path: &str, content: &str) -> io::Result<usize> {
        let abs = self
            .absolute(rel_path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cesta mimo workspace"))?;
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&abs, content)?;
        if let Some(folder) = rel_path.rfind('/').map(|i| rel_path[..i].to_string()) {
            self.register_folder(&folder);
        }

        // Index poznámky se nesmí změnit — UI si ho drží jako odkaz na
        // otevřenou poznámku, backlinky na něj ukazují.
        let index = match self.index_of_path(rel_path) {
            Some(i) => {
                let path = self.notes[i].path.clone();
                self.notes[i] = build_indexed(path, content.to_string());
                i
            }
            None => {
                self.insert(rel_path.to_string(), content.to_string());
                self.notes.len() - 1
            }
        };
        self.rebuild_lookups();
        Ok(index)
    }

    /// Vytvoří novou poznámku. Selže, pokud soubor už existuje.
    ///
    /// Poznámka musí končit na `.txt`. Bez toho by kliknutí na odkaz
    /// v cizí poznámce mohlo uživatele navést k založení jakéhokoli souboru
    /// ve workspace — třeba skriptu nebo konfigurace.
    pub fn create(&mut self, rel_path: &str, content: &str) -> io::Result<usize> {
        if !rel_path
            .rsplit('/')
            .next()
            .map(|name| {
                let lower = name.to_lowercase();
                lower.len() > NOTE_EXT.len() + 1 && lower.ends_with(&format!(".{NOTE_EXT}"))
            })
            .unwrap_or(false)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "poznámka musí mít příponu .txt",
            ));
        }
        let abs = self
            .absolute(rel_path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cesta mimo workspace"))?;
        if abs.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "poznámka už existuje",
            ));
        }
        self.write(rel_path, content)
    }

    /// Rozparsuje poznámku z indexu.
    pub fn parsed(&self, index: usize) -> Option<Note> {
        self.notes.get(index).map(|n| parse(&n.content))
    }

    // --- barvy -------------------------------------------------------------

    fn colors_path(&self) -> PathBuf {
        self.root.join(DATA_DIR).join(COLORS_FILE)
    }

    /// Načte barvy složek. Chybějící nebo poškozený soubor není chyba —
    /// barvy jsou ozdoba, ne data, bez kterých by workspace nešel otevřít.
    fn load_folder_colors(&self) -> std::collections::BTreeMap<String, String> {
        let mut out = std::collections::BTreeMap::new();
        let Ok(text) = fs::read_to_string(self.colors_path()) else {
            return out;
        };

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((path, color)) = line.split_once('=') else {
                continue;
            };
            let path = path.trim();
            let color = color.trim().to_lowercase();
            if !path.is_empty() && COLORS.contains(&color.as_str()) {
                out.insert(path.to_string(), color);
            }
        }
        out
    }

    fn save_folder_colors(&self) -> io::Result<()> {
        let path = self.colors_path();
        if self.folder_colors.is_empty() {
            // Prázdný soubor nemá smysl udržovat.
            if path.exists() {
                fs::remove_file(&path)?;
            }
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut text = String::from(
            "# Barvy složek pro aplikaci Textura.\n\
             # Řádky ve tvaru: cesta = barva\n\
             # Poznámky si barvu nesou samy v property @color.\n\n",
        );
        for (folder, color) in &self.folder_colors {
            text.push_str(&format!("{folder} = {color}\n"));
        }
        fs::write(path, text)
    }

    /// Barva složky, pokud je nastavená.
    pub fn folder_color(&self, folder: &str) -> Option<&str> {
        self.folder_colors.get(folder).map(String::as_str)
    }

    /// Všechny barvy složek.
    pub fn folder_colors(&self) -> Vec<(&str, &str)> {
        self.folder_colors
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    /// Nastaví nebo zruší barvu složky.
    pub fn set_folder_color(&mut self, folder: &str, color: Option<&str>) -> io::Result<()> {
        let folder = folder.trim().trim_end_matches('/').to_string();
        if !self.folders.contains(&folder) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("složka {folder} neexistuje"),
            ));
        }
        match check_color(color)? {
            Some(color) => self.folder_colors.insert(folder, color),
            None => self.folder_colors.remove(&folder),
        };
        self.save_folder_colors()
    }

    /// Nastaví nebo zruší barvu poznámky zápisem property `@color`.
    pub fn set_note_color(&mut self, rel_path: &str, color: Option<&str>) -> io::Result<usize> {
        let color = check_color(color)?;
        let content = self.read(rel_path)?;
        let updated = crate::properties::set(&content, COLOR_PROPERTY, color.as_deref());
        self.write(rel_path, &updated)
    }

    // --- mazání ------------------------------------------------------------

    /// Poznámky ve složce a všech jejích podsložkách.
    pub fn notes_in_folder(&self, folder: &str) -> Vec<usize> {
        let prefix = format!("{}/", folder.trim_end_matches('/'));
        self.notes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.path.starts_with(&prefix))
            .map(|(i, _)| i)
            .collect()
    }

    /// Cíl v koši pro danou cestu. Zajistí, že se nic nepřepíše.
    fn trash_target(&self, rel_path: &str) -> io::Result<PathBuf> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let base = self.root.join(DATA_DIR).join(TRASH_DIR);
        let mut dest = base.join(format!("{stamp}")).join(rel_path);
        let mut counter = 1;
        while dest.exists() {
            dest = base.join(format!("{stamp}-{counter}")).join(rel_path);
            counter += 1;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(dest)
    }

    /// Přesune poznámku do koše. Vrací cestu v koši relativně ke kořeni.
    pub fn delete_note(&mut self, rel_path: &str) -> io::Result<String> {
        let abs = self
            .absolute(rel_path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cesta mimo workspace"))?;
        if !abs.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("poznámka {rel_path} neexistuje"),
            ));
        }

        let dest = self.trash_target(rel_path)?;
        fs::rename(&abs, &dest)?;

        if let Some(i) = self.index_of_path(rel_path) {
            self.notes.remove(i);
            self.rebuild_lookups();
        }

        Ok(relative_to(&self.root, &dest))
    }

    /// Přesune složku i s obsahem do koše.
    ///
    /// Odmítne neprázdnou složku, pokud není `recursive` — smazání složky
    /// s poznámkami má být vědomé rozhodnutí, ne vedlejší efekt překliku.
    pub fn delete_folder(&mut self, rel_path: &str, recursive: bool) -> io::Result<String> {
        let folder = rel_path.trim().trim_end_matches('/');
        let abs = self
            .absolute(folder)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cesta mimo workspace"))?;
        if !abs.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("složka {folder} neexistuje"),
            ));
        }

        if !recursive && fs::read_dir(&abs)?.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::DirectoryNotEmpty,
                "složka není prázdná",
            ));
        }

        let dest = self.trash_target(folder)?;
        fs::rename(&abs, &dest)?;

        // Barvy smazaných složek by jinak zůstaly v souboru napořád.
        let prefix = format!("{folder}/");
        self.folder_colors
            .retain(|path, _| path != folder && !path.starts_with(&prefix));
        self.save_folder_colors()?;

        *self = Workspace::scan(&self.root)?;
        Ok(relative_to(&self.root, &dest))
    }

    // --- přesun ------------------------------------------------------------

    /// Přesune poznámku na jinou cestu (a tím ji případně i přejmenuje).
    ///
    /// Odkazy s explicitní cestou se přepíšou, aby nezůstaly rozbité. Odkazy
    /// holým názvem se nechávají — pokud je název dál jednoznačný, fungují.
    pub fn move_note(&mut self, from: &str, to: &str) -> io::Result<MoveReport> {
        let from_abs = self
            .absolute(from)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "zdroj mimo workspace"))?;
        let to_abs = self
            .absolute(to)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cíl mimo workspace"))?;

        if !to.to_lowercase().ends_with(&format!(".{NOTE_EXT}")) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "poznámka musí mít příponu .txt",
            ));
        }
        if !from_abs.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("poznámka {from} neexistuje"),
            ));
        }
        if from_abs == to_abs {
            return Ok(MoveReport::default());
        }
        if to_abs.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{to} už existuje"),
            ));
        }

        let bare_links = self.count_bare_links(strip_ext(from).rsplit('/').next().unwrap_or(""));

        if let Some(parent) = to_abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&from_abs, &to_abs)?;

        let mapping = vec![(strip_ext(from).to_string(), strip_ext(to).to_string())];
        let mut report = self.rewrite_explicit_links(&mapping)?;
        report.bare_links = bare_links;

        *self = Workspace::scan(&self.root)?;
        Ok(report)
    }

    /// Přesune složku pod jinou složku, čímž z ní udělá podsložku.
    pub fn move_folder(&mut self, from: &str, to: &str) -> io::Result<MoveReport> {
        let source = from.trim().trim_end_matches('/');
        let target = to.trim().trim_end_matches('/');

        let from_abs = self
            .absolute(source)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "zdroj mimo workspace"))?;
        let to_abs = self
            .absolute(target)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cíl mimo workspace"))?;

        if !from_abs.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("složka {source} neexistuje"),
            ));
        }
        if source == target {
            return Ok(MoveReport::default());
        }
        // Složku nelze zanořit do sebe sama ani do vlastní podsložky —
        // fs::rename by v takovém případě strom rozbil.
        if target.starts_with(&format!("{source}/")) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "složku nelze přesunout do vlastní podsložky",
            ));
        }
        if to_abs.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{target} už existuje"),
            ));
        }

        // Mapa starých a nových cest poznámek se musí spočítat před přesunem.
        let prefix = format!("{source}/");
        let mapping: Vec<(String, String)> = self
            .notes
            .iter()
            .filter(|n| n.path.starts_with(&prefix))
            .map(|n| {
                let rest = &n.path[source.len()..];
                (
                    strip_ext(&n.path).to_string(),
                    strip_ext(&format!("{target}{rest}")).to_string(),
                )
            })
            .collect();

        if let Some(parent) = to_abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&from_abs, &to_abs)?;

        // Barva patří složce, ne cestě — po přesunu i přejmenování zůstává.
        let prefix = format!("{source}/");
        self.folder_colors = self
            .folder_colors
            .iter()
            .map(|(path, color)| {
                let moved = if path == source {
                    target.to_string()
                } else if path.starts_with(&prefix) {
                    format!("{target}{}", &path[source.len()..])
                } else {
                    path.clone()
                };
                (moved, color.clone())
            })
            .collect();
        self.save_folder_colors()?;

        let report = self.rewrite_explicit_links(&mapping)?;

        *self = Workspace::scan(&self.root)?;
        Ok(report)
    }

    /// Kolik odkazů v workspace míří na dané jméno bez uvedení cesty.
    fn count_bare_links(&self, name: &str) -> usize {
        let name = lower(name);
        self.notes
            .iter()
            .flat_map(|n| n.links.iter())
            .filter(|l| {
                l.kind == LinkKind::Note && !l.target.contains('/') && lower(&l.target) == name
            })
            .count()
    }

    /// Přepíše odkazy s explicitní cestou podle mapy starých a nových cest.
    ///
    /// Pracuje přímo nad soubory na disku, ne nad indexem — po přesunu je
    /// index zastaralý a stejně se hned potom staví znovu.
    fn rewrite_explicit_links(&self, mapping: &[(String, String)]) -> io::Result<MoveReport> {
        if mapping.is_empty() {
            return Ok(MoveReport::default());
        }
        let lookup: HashMap<String, &str> = mapping
            .iter()
            .map(|(old, new)| (lower(old), new.as_str()))
            .collect();

        let mut paths = Vec::new();
        let mut folders = BTreeSet::new();
        collect_notes(&self.root, &self.root, &mut paths, &mut folders)?;

        let mut report = MoveReport::default();

        for rel in paths {
            let abs = self.root.join(&rel);
            let Ok(bytes) = fs::read(&abs) else {
                continue;
            };
            let content = String::from_utf8_lossy(&bytes).into_owned();
            let note = parse(&content);

            // Od konce, aby dřívější spany zůstaly platné.
            let mut updated = content.clone();
            let mut count = 0usize;
            for link in note.links.iter().rev() {
                if link.kind != LinkKind::Note || !link.target.contains('/') {
                    continue;
                }
                let Some(new_target) = lookup.get(&lower(&link.target)) else {
                    continue;
                };
                updated.replace_range(
                    link.span.start..link.span.end,
                    &render_link(link, new_target),
                );
                count += 1;
            }

            if count > 0 {
                fs::write(&abs, &updated)?;
                report.updated_links += count;
                report.updated_notes += 1;
            }
        }

        Ok(report)
    }
}

/// Cesta bez přípony `.txt` — tvar, kterým na poznámku míří odkaz.
fn strip_ext(path: &str) -> &str {
    path.strip_suffix(&format!(".{NOTE_EXT}")).unwrap_or(path)
}

/// Cesta relativně ke kořeni, s `/`.
fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

/// Poskládá zápis odkazu s novým cílem a zachová kotvu, popisek i embed.
fn render_link(link: &WikiLink, target: &str) -> String {
    let mut out = String::new();
    if link.embed {
        out.push('!');
    }
    out.push_str("[[");
    out.push_str(target);
    match &link.anchor {
        Some(crate::links::Anchor::Section(section)) => {
            out.push('#');
            out.push_str(section);
        }
        Some(crate::links::Anchor::BlockId(id)) => {
            out.push_str("#^");
            out.push_str(id);
        }
        None => {}
    }
    if let Some(label) = &link.label {
        out.push('|');
        out.push_str(label);
    }
    out.push_str("]]");
    out
}

/// Postaví položku indexu z cesty a obsahu.
fn build_indexed(rel_path: String, content: String) -> IndexedNote {
    let note = parse(&content);
    let file_name = rel_path.rsplit('/').next().unwrap_or(&rel_path);
    let name = file_name
        .strip_suffix(&format!(".{NOTE_EXT}"))
        .unwrap_or(file_name)
        .to_string();
    let folder = match rel_path.rfind('/') {
        Some(i) => rel_path[..i].to_string(),
        None => String::new(),
    };

    IndexedNote {
        title: note.title().filter(|t| !t.is_empty()).unwrap_or(&name).to_string(),
        aliases: note.aliases(),
        tags: note.all_tags(),
        links: note.links.clone(),
        // Neznámou hodnotu ignorujeme — uživatel mohl do souboru napsat
        // cokoli a UI nemá co dosazovat za barvu, kterou nezná.
        color: note
            .property(COLOR_PROPERTY)
            .map(|p| p.value.trim().to_lowercase())
            .filter(|c| COLORS.contains(&c.as_str())),
        path: rel_path,
        name,
        folder,
        content,
    }
}

fn collect_notes(
    root: &Path,
    dir: &Path,
    out: &mut Vec<String>,
    folders: &mut BTreeSet<String>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        // Skryté složky a soubory do workspace nepatří (.txtnote, .git, …).
        if name.starts_with('.') {
            continue;
        }

        // Symlinky se do indexu neberou vůbec — jinak by stačilo do workspace
        // podstrčit odkaz a aplikace by přes něj sahala ven.
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if let Ok(rel) = path.strip_prefix(root) {
                folders.insert(rel.to_string_lossy().replace('\\', "/"));
            }
            collect_notes(root, &path, out, folders)?;
        } else if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case(NOTE_EXT))
            .unwrap_or(false)
        {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}
