//! Datové typy posílané do frontendu.
//!
//! Všechny offsety jsou UTF-16 (viz [`crate::offsets`]), aby je CodeMirror
//! mohl použít přímo.

use serde::Serialize;
use textura_core::links::Anchor;
use textura_core::workspace::{IndexedNote, Resolution, Workspace};
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

/// Kompletní rozbor poznámky pro zvýrazňování v editoru.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub properties: Vec<PropertyMarker>,
    pub sections: Vec<Marker>,
    pub links: Vec<LinkMarker>,
    pub tags: Vec<Marker>,
    pub tasks: Vec<TaskMarker>,
    pub urls: Vec<Marker>,
    pub comments: Vec<Marker>,
}

impl Analysis {
    /// `from` je poznámka, ve které text stojí — určuje, jak se přeloží odkazy.
    pub fn build(note: &Note, content: &str, ws: Option<&Workspace>, from: Option<usize>) -> Self {
        let map = Utf16Map::new(content);

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
