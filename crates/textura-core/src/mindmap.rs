//! Mind mapy zapsané jako obyčejný odsazený seznam.
//!
//! Mapu otevírá řádek `@mapa`, volitelně s názvem, a tvoří ji následující
//! položky seznamu. Odsazení určuje zanoření:
//!
//! ```text
//! @mapa Infrastruktura
//! - Produkční server ^server
//!   - [[Docker]] ^docker
//!     - běží v něm pět služeb
//!   - [[MariaDB|databáze]] ^db
//! @spoje
//! - docker -- db
//! ```
//!
//! Uzel je prostý text, takže do něj patří i wiki odkaz — parsuje se běžnou
//! cestou, takže `[[Docker]]` v mapě je normální odkaz a počítá se do
//! backlinků. Mapa proto zůstává čitelná i ve `vim`u a dá se najít
//! `grep -n '^@mapa'`.
//!
//! Uzel může mít stabilní ID `^id` jako sekce. Na ID se odkazují ruční spoje
//! v bloku `@spoje` — čáry, které nevyplývají ze zanoření, protože spojují
//! kteroukoli buňku s kteroukoli jinou.
//!
//! Barva buňky se píše jako `@color:blue`, tedy stejným zápisem, jakým si
//! barvu nese celá poznámka. Ukládá se název barvy, ne odstín — ten dosazuje
//! motiv, aby buňka zůstala čitelná ve světlém i tmavém.
//!
//! Seznam uzlů končí prvním řádkem, který není položka seznamu — včetně
//! prázdného. Blok `@spoje` smí následovat hned za ním a patří k poslední mapě.

use crate::text::{Line, Span};

/// Klíčová slova, kterými smí mapa začínat (case-insensitive, bez dvojtečky —
/// s dvojtečkou na začátku souboru by šlo o property poznámky).
pub const MARKERS: [&str; 2] = ["mapa", "mindmap"];
/// Klíčová slova bloku ručních spojů.
pub const EDGE_MARKERS: [&str; 2] = ["spoje", "edges"];
/// Oddělovač dvou ID ve spoji: `docker -- db`.
pub const EDGE_SEPARATOR: &str = "--";
/// Atribut barvy buňky: `@color:blue`.
pub const COLOR_ATTRIBUTE: &str = "@color:";
/// Nejdelší automaticky vyrobené ID.
const ID_LIMIT: usize = 24;

/// Jeden uzel mapy. `parent` a `children` jsou indexy do [`MindMap::nodes`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindNode {
    /// Text uzlu za `- ` bez případného `^id`, v původním zápisu (tedy včetně
    /// `[[odkazu]]`).
    pub text: String,
    pub text_span: Span,
    /// Stabilní ID bez `^`, pokud ho uzel má. Odkazují se na něj ruční spoje.
    pub id: Option<String>,
    /// Název barvy z `@color:`, pokud je uvedená. Hodnotu mimo paletu si
    /// aplikace zahodí až při vykreslení — parser text nesoudí.
    pub color: Option<String>,
    /// Celý řádek uzlu.
    pub span: Span,
    /// Zanoření: 0 je kořen mapy.
    pub depth: usize,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub line: usize,
}

/// Ruční spoj mezi dvěma uzly — čára, která nevyplývá ze zanoření.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindEdge {
    /// ID uzlů, jak jsou v souboru. Nemusí existovat — soubor mohl někdo
    /// upravit ručně a spoj na zmizelý uzel se prostě nekreslí.
    pub from: String,
    pub to: String,
    /// Indexy do [`MindMap::nodes`], pokud se ID podařilo dohledat.
    pub from_node: Option<usize>,
    pub to_node: Option<usize>,
    pub span: Span,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindMap {
    /// Název mapy za `@mapa`, pokud je uvedený.
    pub title: Option<String>,
    pub title_span: Option<Span>,
    /// Řádek `@mapa …`.
    pub header_span: Span,
    /// Celý blok: hlavička, uzly i případné `@spoje`.
    pub span: Span,
    pub line: usize,
    pub nodes: Vec<MindNode>,
    pub edges: Vec<MindEdge>,
}

impl MindMap {
    /// Kořenové uzly mapy. Map může mít víc kořenů — je to les, ne strom.
    pub fn roots(&self) -> impl Iterator<Item = usize> + '_ {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.parent.is_none())
            .map(|(i, _)| i)
    }

    /// Uzel podle stabilního ID, case-insensitive.
    pub fn node_by_id(&self, id: &str) -> Option<usize> {
        let id = id.to_lowercase();
        self.nodes
            .iter()
            .position(|n| n.id.as_ref().is_some_and(|own| own.to_lowercase() == id))
    }
}

fn is_id_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-')
}

struct Header {
    title: Option<String>,
    title_rel: usize,
}

/// Rozpozná řádek `@mapa` / `@mapa Název`. `markers` říká, o který blok jde.
fn parse_marker_line(text: &str, markers: &[&str]) -> Option<Header> {
    let lead = text.len() - text.trim_start().len();
    let rest = text.trim().strip_prefix('@')?;

    let word_len = rest
        .char_indices()
        .find(|(_, ch)| ch.is_whitespace())
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    let word = &rest[..word_len];
    if !markers.contains(&word.to_lowercase().as_str()) {
        return None;
    }

    let after = &rest[word_len..];
    let title = after.trim();
    if title.is_empty() {
        return Some(Header {
            title: None,
            title_rel: 0,
        });
    }
    Some(Header {
        title: Some(title.to_string()),
        title_rel: lead + 1 + word_len + (after.len() - after.trim_start().len()),
    })
}

/// Odsazení v „sloupcích“; tabulátor se počítá za čtyři, aby mapa držela
/// zanoření i v souboru odsazeném tabulátory.
fn indent_width(text: &str) -> usize {
    text.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// Text položky seznamu `- text`, pokud řádek položkou je.
fn item_text(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    let rest = trimmed.strip_prefix('-')?;
    if rest.is_empty() {
        return Some("");
    }
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some(rest.trim())
}

/// Oddělí od textu uzlu koncový atribut `@color:barva`.
fn split_color(text: &str) -> (&str, Option<&str>) {
    let (head, last) = match text.rsplit_once(char::is_whitespace) {
        Some(pair) => pair,
        None => ("", text),
    };
    match last.strip_prefix(COLOR_ATTRIBUTE) {
        Some(color) if !color.is_empty() && color.chars().all(is_id_char) => {
            (head.trim_end(), Some(color))
        }
        _ => (text, None),
    }
}

/// Oddělí od textu uzlu koncové `^id`, stejně jako u nadpisu sekce.
fn split_id(text: &str) -> (&str, Option<&str>) {
    match text.rsplit_once('^') {
        Some((head, id))
            if !id.is_empty()
                && id.chars().all(is_id_char)
                && head.ends_with(' ')
                && !head.trim().is_empty() =>
        {
            (head.trim_end(), Some(id))
        }
        _ => (text, None),
    }
}

/// Poskládá uzly bloku od hlavičky dál. Vrací uzly a index prvního řádku
/// za blokem.
fn parse_nodes(lines: &[Line<'_>], start: usize) -> (Vec<MindNode>, usize) {
    let mut nodes: Vec<MindNode> = Vec::new();
    // Zásobník (odsazení, index uzlu) — z něj vychází rodič další položky.
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = &lines[i];
        let Some(item) = item_text(line.text) else {
            break;
        };
        let (text, id) = split_id(item);
        let (text, color) = split_color(text);

        let indent = indent_width(line.text);
        while stack.last().is_some_and(|(prev, _)| *prev >= indent) {
            stack.pop();
        }
        let parent = stack.last().map(|(_, index)| *index);
        let index = nodes.len();

        // Offset textu se hledá zprava: `- [[A]] - B` má text `[[A]] - B`.
        let text_rel = if item.is_empty() {
            line.text.len()
        } else {
            line.text.len() - item.len()
        };

        nodes.push(MindNode {
            text: text.to_string(),
            text_span: Span::new(line.abs(text_rel), line.abs(text_rel + text.len())),
            id: id.map(str::to_string),
            color: color.map(str::to_string),
            span: line.span(),
            depth: stack.len(),
            parent,
            children: Vec::new(),
            line: line.number,
        });
        if let Some(p) = parent {
            nodes[p].children.push(index);
        }
        stack.push((indent, index));
        i += 1;
    }

    (nodes, i)
}

/// Poskládá spoje z bloku `@spoje`. Vrací spoje a index prvního řádku za ním.
fn parse_edges(lines: &[Line<'_>], start: usize) -> (Vec<MindEdge>, usize) {
    let mut out = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = &lines[i];
        let Some(item) = item_text(line.text) else {
            break;
        };
        i += 1;

        let Some((from, to)) = item.split_once(EDGE_SEPARATOR) else {
            continue;
        };
        let (from, to) = (from.trim(), to.trim());
        if from.is_empty() || to.is_empty() {
            continue;
        }
        out.push(MindEdge {
            from: from.to_string(),
            to: to.to_string(),
            from_node: None,
            to_node: None,
            span: line.span(),
            line: line.number,
        });
    }

    (out, i)
}

pub fn parse(lines: &[Line<'_>]) -> Vec<MindMap> {
    let mut out = Vec::new();
    let mut i = 0usize;

    while i < lines.len() {
        let line = &lines[i];
        let Some(header) = parse_marker_line(line.text, &MARKERS) else {
            i += 1;
            continue;
        };

        let (nodes, after_nodes) = parse_nodes(lines, i + 1);
        let mut end = nodes
            .last()
            .map(|n| n.span.end)
            .unwrap_or_else(|| line.end());

        // Blok spojů patří k mapě, takže je součástí jejího rozsahu — UI mapu
        // přepisuje celou a spoje by se jinak zdvojily.
        let mut edges = Vec::new();
        let mut next = after_nodes;
        if let Some(edge_header) = lines.get(after_nodes) {
            if parse_marker_line(edge_header.text, &EDGE_MARKERS).is_some() {
                let (parsed, after_edges) = parse_edges(lines, after_nodes + 1);
                end = parsed
                    .last()
                    .map(|e| e.span.end)
                    .unwrap_or_else(|| edge_header.end());
                edges = parsed;
                next = after_edges;
            }
        }

        let mut map = MindMap {
            title_span: header.title.as_ref().map(|t| {
                Span::new(line.abs(header.title_rel), line.abs(header.title_rel + t.len()))
            }),
            title: header.title,
            header_span: line.span(),
            span: Span::new(line.start, end),
            line: line.number,
            nodes,
            edges,
        };

        for index in 0..map.edges.len() {
            let (from, to) = (map.edges[index].from.clone(), map.edges[index].to.clone());
            map.edges[index].from_node = map.node_by_id(&from);
            map.edges[index].to_node = map.node_by_id(&to);
        }

        out.push(map);
        i = next.max(i + 1);
    }

    out
}

/// Uzel k zápisu: zanoření, text a případná barva a stabilní ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeSpec {
    pub depth: usize,
    pub text: String,
    pub color: Option<String>,
    pub id: Option<String>,
}

/// Poskládá text bloku mapy — uzly i případné ruční spoje.
///
/// Zanoření se srovnává stejným zásobníkem, jakým ho čte parser: první uzel je
/// vždy kořen a mezery v úrovních se zavřou, takže z UI nemůže vzniknout blok,
/// který se po znovunačtení rozpadne jinak, než jak byl poslán.
pub fn render(
    title: Option<&str>,
    nodes: &[NodeSpec],
    edges: &[(String, String)],
    line_ending: &str,
) -> String {
    let mut out = String::new();
    out.push_str("@mapa");
    if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
        out.push(' ');
        out.push_str(title);
    }

    let mut stack: Vec<usize> = Vec::new();
    for node in nodes {
        while stack.last().is_some_and(|top| *top >= node.depth) {
            stack.pop();
        }
        let depth = stack.len();
        stack.push(node.depth);

        out.push_str(line_ending);
        for _ in 0..depth {
            out.push_str("  ");
        }
        out.push_str("- ");
        out.push_str(node.text.trim());
        if let Some(color) = node.color.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
            out.push(' ');
            out.push_str(COLOR_ATTRIBUTE);
            out.push_str(color);
        }
        if let Some(id) = node.id.as_deref().map(str::trim).filter(|i| !i.is_empty()) {
            out.push_str(" ^");
            out.push_str(id);
        }
    }

    if !edges.is_empty() {
        out.push_str(line_ending);
        out.push_str("@spoje");
        for (from, to) in edges {
            out.push_str(line_ending);
            out.push_str("- ");
            out.push_str(from.trim());
            out.push(' ');
            out.push_str(EDGE_SEPARATOR);
            out.push(' ');
            out.push_str(to.trim());
        }
    }

    out
}

/// Vyrobí stabilní ID z textu uzlu. `taken` jsou ID, která už jsou zabraná.
///
/// ID musí být čitelné i v souboru, takže vychází z textu uzlu: `[[Docker]]`
/// dá `docker`. Diakritika se skládá stejně jako při hledání, aby `Žluťoučký`
/// dal `zlutoucky` a ID zůstalo psatelné na klávesnici.
pub fn suggest_id(text: &str, taken: &[String]) -> String {
    let folded = crate::search::fold(text);
    let mut base = String::new();
    for ch in folded.chars() {
        if ch.is_ascii_alphanumeric() {
            base.push(ch);
        } else if !base.ends_with('-') {
            base.push('-');
        }
        if base.len() >= ID_LIMIT {
            break;
        }
    }
    let base = base.trim_matches('-').to_string();
    let base = if base.is_empty() {
        "uzel".to_string()
    } else {
        base
    };

    let is_free = |candidate: &str| {
        !taken
            .iter()
            .any(|t| t.to_lowercase() == candidate.to_lowercase())
    };
    if is_free(&base) {
        return base;
    }
    for n in 2..1000 {
        let candidate = format!("{base}-{n}");
        if is_free(&candidate) {
            return candidate;
        }
    }
    base
}
