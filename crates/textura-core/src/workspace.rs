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
use crate::text::Span;
use crate::{parse, Note};

/// Přípona poznámky.
pub const NOTE_EXT: &str = "txt";
/// Složka s technickými metadaty aplikace.
pub const DATA_DIR: &str = ".txtnote";

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

/// Nález fulltextu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub path: String,
    pub line: usize,
    /// Řádek s nálezem, ořezaný o okolní whitespace.
    pub snippet: String,
    /// Rozsah nálezu v obsahu poznámky.
    pub span: Span,
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
    pub fn absolute(&self, rel_path: &str) -> Option<PathBuf> {
        is_safe_relative(rel_path).then(|| self.root.join(rel_path))
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

    /// Fulltext přes obsah poznámek, case-insensitive.
    ///
    /// `limit` omezuje počet nálezů, aby UI nedostalo desetitisíce řádků.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let needle = lower(query.trim());
        if needle.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::new();
        'notes: for note in &self.notes {
            let haystack = lower(&note.content);
            // `to_lowercase` může změnit délku, takže offsety hledáme znovu
            // nad původním textem po řádcích — tím zůstanou spany platné.
            if !haystack.contains(&needle) {
                continue;
            }
            let mut offset = 0usize;
            for (n, line) in note.content.split_inclusive('\n').enumerate() {
                let line_lower = lower(line);
                if let Some(rel) = line_lower.find(&needle) {
                    // Pozici přepočítáme přes počet znaků, ne bytů.
                    let chars_before = line_lower[..rel].chars().count();
                    let start = line
                        .char_indices()
                        .nth(chars_before)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    let end = line
                        .char_indices()
                        .nth(chars_before + query.trim().chars().count())
                        .map(|(i, _)| i)
                        .unwrap_or(line.len());
                    out.push(SearchHit {
                        path: note.path.clone(),
                        line: n + 1,
                        snippet: line.trim().to_string(),
                        span: Span::new(offset + start, offset + end),
                    });
                    if out.len() >= limit {
                        break 'notes;
                    }
                }
                offset += line.len();
            }
        }
        out
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
    pub fn create(&mut self, rel_path: &str, content: &str) -> io::Result<usize> {
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

        let file_type = entry.file_type()?;
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
