//! Datové typy posílané do frontendu.
//!
//! Všechny offsety jsou UTF-16 (viz [`crate::offsets`]), aby je CodeMirror
//! mohl použít přímo.

use serde::Serialize;
use textura_core::links::Anchor;
use textura_core::workspace::{HitKind, IndexedNote, Resolution, Workspace};
use textura_core::{LinkKind, Note, TaskState};

use crate::offsets::Utf16Map;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub root: String,
    pub note_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub path: String,
    pub name: String,
    pub title: String,
    pub folder: String,
    pub tags: Vec<String>,
    pub color: Option<String>,
}

impl NoteSummary {
    pub fn from(note: &IndexedNote) -> Self {
        Self {
            path: note.path.clone(),
            name: note.name.clone(),
            title: note.title.clone(),
            folder: note.folder.clone(),
            tags: note.tags.clone(),
            color: note.color.clone(),
        }
    }
}

/// Složka pro strom v UI.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    pub path: String,
    pub color: Option<String>,
}

/// Jeden zvýrazněný úsek textu.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub from: u32,
    pub to: u32,
    /// Doplňkový text podle druhu markeru (název tagu, URL, nadpis sekce).
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkMarker {
    pub from: u32,
    pub to: u32,
    /// Obsah mezi `[[` a `]]` — klíč pro `follow_link`.
    pub raw: String,
    pub target: String,
    /// Text, který se má uživateli zobrazit.
    pub display: String,
    pub embed: bool,
    /// `note` / `attachment`
    pub kind: &'static str,
    /// `resolved` / `ambiguous` / `unresolved`
    pub state: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMarker {
    pub from: u32,
    pub to: u32,
    /// Rozsah samotného `[ ]`, aby šlo stav překliknout.
    pub marker_from: u32,
    pub marker_to: u32,
    /// `open` / `done` / `important` / `other`
    pub state: &'static str,
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyMarker {
    pub from: u32,
    pub to: u32,
    pub key: String,
    pub value: String,
}

/// Uzel mind mapy. `link` je index do [`Analysis::links`], pokud uzel obsahuje
/// wiki odkaz — mapa žádné vlastní odkazy neparsuje, jen ukazuje na ty běžné.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MindNodeMarker {
    pub from: u32,
    pub to: u32,
    pub text_from: u32,
    pub text_to: u32,
    pub depth: usize,
    pub parent: Option<usize>,
    /// Text uzlu, jak je v souboru — včetně `[[odkazů]]`.
    pub text: String,
    /// Text k vykreslení: odkazy nahrazené tím, co má uživatel vidět.
    pub label: String,
    /// Stabilní ID `^id`, pokud ho uzel má. Bez ID nelze uzel ručně spojit
    /// ani mu uložit pozici — UI si ho v takovém případě nechá vyrobit.
    pub id: Option<String>,
    /// Barva buňky z `@color:`; hodnotu mimo paletu UI ignoruje.
    pub color: Option<String>,
    pub link: Option<usize>,
    /// Uložená pozice bubliny z `.txtnote/mindmap.txt`, jinak `null` a bublinu
    /// si mapa srovná sama.
    pub x: Option<i32>,
    pub y: Option<i32>,
}

/// Ruční spoj mezi dvěma buňkami — čára, která nevyplývá ze zanoření.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MindEdgeMarker {
    pub from: String,
    pub to: String,
    /// Indexy uzlů, pokud se ID podařilo dohledat; jinak spoj nikam nemíří.
    pub from_node: Option<usize>,
    pub to_node: Option<usize>,
    pub line: usize,
}

/// Blok `@mapa` s odsazeným seznamem uzlů.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MindMapMarker {
    /// Celý blok — UI ho při úpravě mapy vymění za nový text.
    pub from: u32,
    pub to: u32,
    pub header_from: u32,
    pub header_to: u32,
    pub title: Option<String>,
    pub line: usize,
    pub nodes: Vec<MindNodeMarker>,
    pub edges: Vec<MindEdgeMarker>,
}

/// Kompletní rozbor poznámky pro zvýrazňování v editoru.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub properties: Vec<PropertyMarker>,
    pub sections: Vec<Marker>,
    pub mindmaps: Vec<MindMapMarker>,
    pub links: Vec<LinkMarker>,
    pub tags: Vec<Marker>,
    pub tasks: Vec<TaskMarker>,
    pub urls: Vec<Marker>,
    pub comments: Vec<Marker>,
}

impl Analysis {
    /// `from` je poznámka, ve které text stojí — určuje, jak se přeloží odkazy.
    /// `positions` jsou uložené pozice bublin mind map dané poznámky.
    pub fn build(note: &Note, content: &str, ws: Option<&Workspace>, from: Option<usize>) -> Self {
        Self::build_with_layout(note, content, ws, from, &[])
    }

    pub fn build_with_layout(
        note: &Note,
        content: &str,
        ws: Option<&Workspace>,
        from: Option<usize>,
        positions: &[(&str, (i32, i32))],
    ) -> Self {
        let map = Utf16Map::new(content);
        let position_of = |id: Option<&String>| {
            id.and_then(|id| {
                positions
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case(id))
                    .map(|(_, pos)| *pos)
            })
        };

        let links = note
            .links
            .iter()
            .map(|link| {
                let state = match ws.map(|ws| ws.resolve(link, from)) {
                    Some(Resolution::Note(_)) | Some(Resolution::Attachment(_)) => "resolved",
                    Some(Resolution::Ambiguous(_)) => "ambiguous",
                    Some(Resolution::Unresolved { .. }) => "unresolved",
                    // Bez workspace (např. náhled) odkazy nevyhodnocujeme.
                    None => "resolved",
                };
                LinkMarker {
                    from: map.at(link.span.start),
                    to: map.at(link.span.end),
                    raw: link.raw.clone(),
                    target: link.target.clone(),
                    display: link.display_text(),
                    embed: link.embed,
                    kind: match link.kind {
                        LinkKind::Note => "note",
                        LinkKind::Attachment => "attachment",
                    },
                    state,
                }
            })
            .collect();

        let mindmaps = note
            .mindmaps
            .iter()
            .map(|mm| MindMapMarker {
                from: map.at(mm.span.start),
                to: map.at(mm.span.end),
                header_from: map.at(mm.header_span.start),
                header_to: map.at(mm.header_span.end),
                title: mm.title.clone(),
                line: mm.line,
                nodes: mm
                    .nodes
                    .iter()
                    .map(|node| MindNodeMarker {
                        from: map.at(node.span.start),
                        to: map.at(node.span.end),
                        text_from: map.at(node.text_span.start),
                        text_to: map.at(node.text_span.end),
                        depth: node.depth,
                        parent: node.parent,
                        label: node_label(note, node),
                        text: node.text.clone(),
                        id: node.id.clone(),
                        color: node.color.clone(),
                        x: position_of(node.id.as_ref()).map(|(x, _)| x),
                        y: position_of(node.id.as_ref()).map(|(_, y)| y),
                        // Odkaz uzlu je obyčejný odkaz poznámky — jen se na něj
                        // ukáže indexem, aby stav (existuje / nejednoznačný)
                        // nebyl v analýze dvakrát.
                        link: note
                            .links
                            .iter()
                            .position(|l| l.span.inside(&node.text_span)),
                    })
                    .collect(),
                edges: mm
                    .edges
                    .iter()
                    .map(|edge| MindEdgeMarker {
                        from: edge.from.clone(),
                        to: edge.to.clone(),
                        from_node: edge.from_node,
                        to_node: edge.to_node,
                        line: edge.line,
                    })
                    .collect(),
            })
            .collect();

        Self {
            properties: note
                .properties
                .iter()
                .map(|p| PropertyMarker {
                    from: map.at(p.span.start),
                    to: map.at(p.span.end),
                    key: p.key.clone(),
                    value: p.value.clone(),
                })
                .collect(),
            sections: note
                .sections
                .iter()
                .map(|s| Marker {
                    from: map.at(s.span.start),
                    to: map.at(s.span.end),
                    text: s.title.clone(),
                })
                .collect(),
            links,
            mindmaps,
            tags: note
                .tags
                .iter()
                .map(|t| Marker {
                    from: map.at(t.span.start),
                    to: map.at(t.span.end),
                    text: t.name.clone(),
                })
                .collect(),
            tasks: note
                .tasks
                .iter()
                .map(|t| TaskMarker {
                    from: map.at(t.span.start),
                    to: map.at(t.span.end),
                    marker_from: map.at(t.marker_span.start),
                    marker_to: map.at(t.marker_span.end),
                    state: match t.state {
                        TaskState::Open => "open",
                        TaskState::Done => "done",
                        TaskState::Important => "important",
                        TaskState::Other(_) => "other",
                    },
                    text: t.text.clone(),
                })
                .collect(),
            urls: note
                .urls
                .iter()
                .map(|u| Marker {
                    from: map.at(u.span.start),
                    to: map.at(u.span.end),
                    text: u.url.clone(),
                })
                .collect(),
            comments: note
                .comments
                .iter()
                .map(|c| Marker {
                    from: map.at(c.span.start),
                    to: map.at(c.span.end),
                    text: c.text.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteContent {
    pub path: String,
    pub title: String,
    pub content: String,
    pub analysis: Analysis,
}

/// Kam vede kliknutí na odkaz.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FollowResult {
    /// Otevři poznámku, případně skoč na řádek se sekcí (1-based).
    Note {
        path: String,
        line: Option<usize>,
    },
    /// Nabídni výběr z víc kandidátů.
    Ambiguous { candidates: Vec<NoteSummary> },
    /// Otevři přílohu v systémové aplikaci; cesta je absolutní.
    Attachment { path: String },
    /// Poznámka neexistuje — nabídni její vytvoření.
    Unresolved {
        suggested_path: String,
        title: String,
    },
}

impl FollowResult {
    pub fn from_resolution(
        ws: &Workspace,
        resolution: Resolution,
        anchor: Option<&Anchor>,
        fallback_title: &str,
    ) -> Self {
        match resolution {
            Resolution::Note(i) => {
                let note = ws.note(i).expect("index z resolveru");
                FollowResult::Note {
                    path: note.path.clone(),
                    line: anchor.and_then(|a| section_line(ws, i, a)),
                }
            }
            Resolution::Ambiguous(candidates) => FollowResult::Ambiguous {
                candidates: candidates
                    .iter()
                    .filter_map(|i| ws.note(*i))
                    .map(NoteSummary::from)
                    .collect(),
            },
            Resolution::Attachment(path) => FollowResult::Attachment {
                path: ws
                    .absolute(&path)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or(path),
            },
            Resolution::Unresolved { suggested_path } => FollowResult::Unresolved {
                suggested_path,
                title: fallback_title.to_string(),
            },
        }
    }
}

/// Text uzlu mapy k vykreslení: `[[Docker|kontejnery]]` se ukáže jako
/// `kontejnery`. Mapa kreslí uzly sama, takže hranaté závorky by v ní byly
/// jen šum — v souboru ovšem zůstávají.
fn node_label(note: &Note, node: &textura_core::MindNode) -> String {
    let mut out = String::new();
    let mut cursor = node.text_span.start;
    for link in note.links.iter().filter(|l| l.span.inside(&node.text_span)) {
        out.push_str(&node.text[cursor - node.text_span.start..link.span.start - node.text_span.start]);
        out.push_str(&link.display_text());
        cursor = link.span.end;
    }
    out.push_str(&node.text[cursor - node.text_span.start..]);
    out
}

/// Řádek sekce, na kterou odkaz míří.
fn section_line(ws: &Workspace, note: usize, anchor: &Anchor) -> Option<usize> {
    let parsed = ws.parsed(note)?;
    let section = match anchor {
        Anchor::Section(title) => parsed.section(title),
        Anchor::BlockId(id) => parsed.section_by_id(id),
    }?;
    Some(section.line)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacklinkEntry {
    pub path: String,
    pub title: String,
    pub line: usize,
    pub context: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub path: String,
    pub title: String,
    pub line: usize,
    pub snippet: String,
    /// Kde se nález potkal s dotazem: `title`, `alias` nebo `body`.
    pub kind: &'static str,
    /// UTF-16 rozsah nálezu uvnitř `snippet` — zvýraznění v seznamu výsledků.
    pub snippet_from: u32,
    pub snippet_to: u32,
    /// UTF-16 rozsah nálezu v obsahu poznámky, aby ho editor po otevření mohl
    /// označit. `null`, pokud nález v obsahu není (název z jména souboru).
    pub match_from: Option<u32>,
    pub match_to: Option<u32>,
}

impl SearchResult {
    /// `content_map` je mapa obsahu poznámky, ve které nález leží.
    pub fn from(hit: textura_core::SearchHit, title: String, content_map: &Utf16Map) -> Self {
        let snippet_map = Utf16Map::new(&hit.snippet);
        let (match_from, match_to) = if hit.span.is_empty() {
            (None, None)
        } else {
            (
                Some(content_map.at(hit.span.start)),
                Some(content_map.at(hit.span.end)),
            )
        };

        Self {
            path: hit.path,
            title,
            line: hit.line,
            kind: match hit.kind {
                HitKind::Title => "title",
                HitKind::Alias => "alias",
                HitKind::Body => "body",
            },
            snippet_from: snippet_map.at(hit.snippet_span.start),
            snippet_to: snippet_map.at(hit.snippet_span.end),
            snippet: hit.snippet,
            match_from,
            match_to,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagCount {
    pub tag: String,
    pub count: usize,
}

/// Výsledek přesunu — kolik odkazů se opravilo a na co si dát pozor.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveOutcome {
    /// Nová cesta.
    pub path: String,
    pub updated_links: usize,
    pub updated_notes: usize,
    /// Odkazy holým názvem, které se nepřepisovaly.
    pub bare_links: usize,
}

impl MoveOutcome {
    pub fn from(report: textura_core::workspace::MoveReport, path: &str) -> Self {
        Self {
            path: path.to_string(),
            updated_links: report.updated_links,
            updated_notes: report.updated_notes,
            bare_links: report.bare_links,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Analysis, SearchResult};
    use crate::offsets::Utf16Map;
    use textura_core::workspace::HitKind;
    use textura_core::{parse, SearchHit, Span};

    #[test]
    fn uzel_mapy_se_kresli_bez_hranatych_zavorek() {
        let content = "@mapa Síť\n- [[Docker|kontejnery]] a k tomu [[MariaDB]]\n";
        let note = parse(content);

        let analysis = Analysis::build(&note, content, None, None);
        let map = &analysis.mindmaps[0];
        let node = &map.nodes[0];

        assert_eq!(map.title.as_deref(), Some("Síť"));
        assert_eq!(node.text, "[[Docker|kontejnery]] a k tomu [[MariaDB]]");
        assert_eq!(node.label, "kontejnery a k tomu MariaDB");
        // Odkaz uzlu je odkaz poznámky, jen se na něj ukazuje indexem.
        assert_eq!(node.link, Some(0));
        assert_eq!(analysis.links[node.link.unwrap()].target, "Docker");
    }

    #[test]
    fn mapa_nese_id_spoje_i_ulozene_pozice() {
        let content = "@mapa\n- A ^a\n- B ^b\n@spoje\n- a -- b\n";
        let note = parse(content);

        let analysis =
            Analysis::build_with_layout(&note, content, None, None, &[("A", (120, 40))]);
        let map = &analysis.mindmaps[0];

        assert_eq!(map.nodes[0].id.as_deref(), Some("a"));
        // Pozice se dohledává case-insensitive, ať ruční úprava souboru nevadí.
        assert_eq!((map.nodes[0].x, map.nodes[0].y), (Some(120), Some(40)));
        assert_eq!((map.nodes[1].x, map.nodes[1].y), (None, None));
        assert_eq!(map.edges.len(), 1);
        assert_eq!((map.edges[0].from_node, map.edges[0].to_node), (Some(0), Some(1)));
    }

    #[test]
    fn rozsahy_mapy_jsou_v_utf16() {
        let content = "Žluť\n\n@mapa\n- kůň\n";
        let note = parse(content);

        let analysis = Analysis::build(&note, content, None, None);
        let map = &analysis.mindmaps[0];

        // „Žluť\n\n“ je 6 bytů, ale 6 UTF-16 jednotek jen shodou okolností;
        // uzel `- kůň` má diakritiku, takže se byty a jednotky rozejdou.
        assert_eq!(map.header_from, 6);
        assert_eq!(map.header_to, 11);
        assert_eq!(map.nodes[0].text_from, 14);
        assert_eq!(map.nodes[0].text_to, 17);
    }

    #[test]
    fn nalez_prevede_byty_na_utf16() {
        let content = "Kůň žluťoučký\n";
        let start = content.find("žluťoučký").unwrap();
        let hit = SearchHit {
            path: "A.txt".into(),
            kind: HitKind::Body,
            line: 1,
            snippet: "Kůň žluťoučký".into(),
            span: Span::new(start, start + "žluťoučký".len()),
            snippet_span: Span::new(start, start + "žluťoučký".len()),
        };

        let result = SearchResult::from(hit, "A".into(), &Utf16Map::new(content));

        // Bytů je před nálezem 6 (ů a ň jsou dvoubytové), UTF-16 jednotek 4.
        assert_eq!(start, 6);
        assert_eq!(result.match_from, Some(4));
        assert_eq!(result.match_to, Some(13));
        assert_eq!(result.snippet_from, 4);
        assert_eq!(result.snippet_to, 13);
        assert_eq!(result.kind, "body");
    }

    #[test]
    fn nalez_bez_rozsahu_v_obsahu_nema_match() {
        // Název odvozený od jména souboru v obsahu poznámky není.
        let hit = SearchHit {
            path: "Docker.txt".into(),
            kind: HitKind::Title,
            line: 1,
            snippet: "Docker".into(),
            span: Span::new(0, 0),
            snippet_span: Span::new(0, 6),
        };

        let result = SearchResult::from(hit, "Docker".into(), &Utf16Map::new("obsah\n"));

        assert_eq!(result.match_from, None);
        assert_eq!(result.match_to, None);
        assert_eq!(result.kind, "title");
        assert_eq!((result.snippet_from, result.snippet_to), (0, 6));
    }
}
