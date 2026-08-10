//! Tauri commandy — jediné rozhraní mezi UI a jádrem.

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::State;
use textura_core::workspace::{Resolution, Workspace};
use textura_core::{parse, LineEnding};

use crate::dto::*;
use crate::offsets::Utf16Map;

#[derive(Default)]
pub struct AppState {
    pub workspace: Mutex<Option<Workspace>>,
    /// Složka předaná na příkazové řádce (`textura ~/Poznamky`).
    pub initial: Option<String>,
}

type Res<T> = Result<T, String>;

/// Pomocník, aby se `Mutex` a „workspace není otevřený“ neopakovaly v každém
/// commandu.
fn with_ws<T>(state: &State<AppState>, f: impl FnOnce(&Workspace) -> Res<T>) -> Res<T> {
    let guard = state.workspace.lock().map_err(|e| e.to_string())?;
    let ws = guard.as_ref().ok_or("workspace není otevřený")?;
    f(ws)
}

fn with_ws_mut<T>(state: &State<AppState>, f: impl FnOnce(&mut Workspace) -> Res<T>) -> Res<T> {
    let mut guard = state.workspace.lock().map_err(|e| e.to_string())?;
    let ws = guard.as_mut().ok_or("workspace není otevřený")?;
    f(ws)
}

/// Složka z příkazové řádky; má přednost před naposledy otevřenou.
#[tauri::command]
pub fn initial_workspace(state: State<AppState>) -> Option<String> {
    state.initial.clone()
}

#[tauri::command]
pub fn open_workspace(path: String, state: State<AppState>) -> Res<WorkspaceInfo> {
    let ws = Workspace::scan(&path).map_err(|e| format!("nelze načíst {path}: {e}"))?;
    let info = WorkspaceInfo {
        root: ws.root().to_string_lossy().into_owned(),
        note_count: ws.notes().len(),
    };
    *state.workspace.lock().map_err(|e| e.to_string())? = Some(ws);
    Ok(info)
}

/// Znovu načte celý index z disku — poznámky mohly mezitím změnit jiné
/// nástroje (Vim, Syncthing, git pull).
#[tauri::command]
pub fn reload_workspace(state: State<AppState>) -> Res<WorkspaceInfo> {
    let root = with_ws(&state, |ws| Ok(ws.root().to_path_buf()))?;
    open_workspace(root.to_string_lossy().into_owned(), state)
}

#[tauri::command]
pub fn list_notes(state: State<AppState>) -> Res<Vec<NoteSummary>> {
    with_ws(&state, |ws| {
        let mut notes: Vec<NoteSummary> = ws.notes().iter().map(NoteSummary::from).collect();
        notes.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        Ok(notes)
    })
}

#[tauri::command]
pub fn read_note(path: String, state: State<AppState>) -> Res<NoteContent> {
    with_ws(&state, |ws| {
        // Čteme z disku, ne z indexu — soubor mohl někdo upravit zvenčí.
        let content = ws.read(&path).map_err(|e| format!("{path}: {e}"))?;
        let note = parse(&content);
        let index = ws.index_of_path(&path);
        let title = note
            .title()
            .map(|t| t.to_string())
            .or_else(|| index.and_then(|i| ws.note(i)).map(|n| n.title.clone()))
            .unwrap_or_else(|| path.clone());

        Ok(NoteContent {
            analysis: Analysis::build(&note, &content, Some(ws), index),
            path,
            title,
            content,
        })
    })
}

/// Rozbor rozepsaného textu — volá se při psaní, aby zvýraznění odpovídalo
/// aktuálnímu obsahu editoru.
#[tauri::command]
pub fn analyze(path: String, content: String, state: State<AppState>) -> Res<Analysis> {
    with_ws(&state, |ws| {
        let note = parse(&content);
        Ok(Analysis::build(
            &note,
            &content,
            Some(ws),
            ws.index_of_path(&path),
        ))
    })
}

#[tauri::command]
pub fn save_note(path: String, content: String, state: State<AppState>) -> Res<()> {
    with_ws_mut(&state, |ws| {
        // Zachováme konce řádků a BOM původního souboru — aplikace nesmí
        // uživateli přeformátovat soubor jen tím, že ho otevřela.
        let (line_ending, had_bom) = match ws.read(&path) {
            Ok(existing) => {
                let note = parse(&existing);
                (note.line_ending, note.had_bom)
            }
            Err(_) => (LineEnding::Lf, false),
        };

        let out = apply_file_style(&content, line_ending, had_bom);
        ws.write(&path, &out).map_err(|e| format!("{path}: {e}"))?;
        Ok(())
    })
}

/// Převede text z editoru do stylu původního souboru.
///
/// Editor pracuje vždy s LF a bez BOM. Kdyby se ukládalo takhle, aplikace by
/// uživateli přepsala celý soubor jen tím, že ho otevřel — a v gitu by z toho
/// byl diff přes všechny řádky.
pub(crate) fn apply_file_style(content: &str, line_ending: LineEnding, had_bom: bool) -> String {
    let normalized = content.replace("\r\n", "\n");
    let mut out = match line_ending {
        LineEnding::Crlf => normalized.replace('\n', "\r\n"),
        _ => normalized,
    };
    if had_bom && !out.starts_with('\u{feff}') {
        out.insert(0, '\u{feff}');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::apply_file_style;
    use textura_core::LineEnding;

    #[test]
    fn lf_soubor_zustane_lf() {
        assert_eq!(
            apply_file_style("a\nb\n", LineEnding::Lf, false),
            "a\nb\n"
        );
    }

    #[test]
    fn crlf_soubor_se_ulozi_jako_crlf() {
        assert_eq!(
            apply_file_style("a\nb\n", LineEnding::Crlf, false),
            "a\r\nb\r\n"
        );
    }

    #[test]
    fn crlf_se_nezdvoji() {
        // Editor už mohl poslat CRLF; nesmí z toho vzniknout \r\r\n.
        assert_eq!(
            apply_file_style("a\r\nb\r\n", LineEnding::Crlf, false),
            "a\r\nb\r\n"
        );
    }

    #[test]
    fn bom_se_zachova_a_neprida_dvakrat() {
        assert_eq!(
            apply_file_style("a\n", LineEnding::Lf, true),
            "\u{feff}a\n"
        );
        assert_eq!(
            apply_file_style("\u{feff}a\n", LineEnding::Lf, true),
            "\u{feff}a\n"
        );
    }

    #[test]
    fn soubor_bez_bom_ho_nedostane() {
        assert!(!apply_file_style("a\n", LineEnding::Lf, false).starts_with('\u{feff}'));
    }

    #[test]
    fn smiseny_soubor_se_sjednoti_na_lf() {
        // U smíšeného souboru nemá „původní styl“ jednoznačný význam;
        // LF je bezpečnější volba než míchat dál.
        assert_eq!(
            apply_file_style("a\r\nb\n", LineEnding::Mixed, false),
            "a\nb\n"
        );
    }
}

#[tauri::command]
pub fn create_note(path: String, title: String, state: State<AppState>) -> Res<NoteSummary> {
    with_ws_mut(&state, |ws| {
        // Nová poznámka je čistý text: nadpis a prázdný řádek, nic víc.
        let content = format!("{title}\n\n");
        let index = ws.create(&path, &content).map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => format!("poznámka {path} už existuje"),
            _ => format!("{path}: {e}"),
        })?;
        Ok(NoteSummary::from(ws.note(index).expect("nová poznámka")))
    })
}

/// Přesune poznámku do koše v `.txtnote/trash/`. Vrací cestu v koši.
#[tauri::command]
pub fn delete_note(path: String, state: State<AppState>) -> Res<String> {
    with_ws_mut(&state, |ws| {
        ws.delete_note(&path).map_err(|e| format!("{path}: {e}"))
    })
}

/// Kolik poznámek je ve složce a jejích podsložkách — pro potvrzovací dialog.
#[tauri::command]
pub fn count_notes_in_folder(folder: String, state: State<AppState>) -> Res<usize> {
    with_ws(&state, |ws| Ok(ws.notes_in_folder(&folder).len()))
}

/// Přesune složku i s obsahem do koše. `recursive` musí být `true`,
/// pokud složka není prázdná.
#[tauri::command]
pub fn delete_folder(path: String, recursive: bool, state: State<AppState>) -> Res<String> {
    with_ws_mut(&state, |ws| {
        ws.delete_folder(&path, recursive)
            .map_err(|e| format!("{path}: {e}"))
    })
}

#[tauri::command]
pub fn move_note(from: String, to: String, state: State<AppState>) -> Res<MoveOutcome> {
    with_ws_mut(&state, |ws| {
        let report = ws
            .move_note(&from, &to)
            .map_err(|e| format!("{from} → {to}: {e}"))?;
        Ok(MoveOutcome::from(report, &to))
    })
}

#[tauri::command]
pub fn move_folder(from: String, to: String, state: State<AppState>) -> Res<MoveOutcome> {
    with_ws_mut(&state, |ws| {
        let report = ws
            .move_folder(&from, &to)
            .map_err(|e| format!("{from} → {to}: {e}"))?;
        Ok(MoveOutcome::from(report, &to))
    })
}

/// Slovníky, které jsou v systému k dispozici.
#[tauri::command]
pub fn available_dictionaries() -> Vec<String> {
    crate::spellcheck::available_dictionaries()
}

/// Zapne či vypne kontrolu překlepů a nastaví jazyky.
#[tauri::command]
pub fn set_spell_check(
    enabled: bool,
    languages: Vec<String>,
    window: tauri::WebviewWindow,
) -> Res<()> {
    crate::spellcheck::apply(&window, enabled, languages)
}

fn folder_list(ws: &Workspace) -> Vec<FolderInfo> {
    ws.folders()
        .into_iter()
        .map(|path| FolderInfo {
            color: ws.folder_color(path).map(str::to_string),
            path: path.to_string(),
        })
        .collect()
}

/// Složky workspace, včetně prázdných.
#[tauri::command]
pub fn list_folders(state: State<AppState>) -> Res<Vec<FolderInfo>> {
    with_ws(&state, |ws| Ok(folder_list(ws)))
}

#[tauri::command]
pub fn create_folder(path: String, state: State<AppState>) -> Res<Vec<FolderInfo>> {
    with_ws_mut(&state, |ws| {
        ws.create_folder(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => format!("složka {path} už existuje"),
            std::io::ErrorKind::InvalidInput => format!("neplatný název složky: {path}"),
            _ => format!("{path}: {e}"),
        })?;
        Ok(folder_list(ws))
    })
}

/// Paleta barev, ze které si UI vybírá.
#[tauri::command]
pub fn available_colors() -> Vec<String> {
    textura_core::workspace::COLORS
        .iter()
        .map(|c| c.to_string())
        .collect()
}

/// Nastaví barvu poznámky. `None` barvu zruší.
#[tauri::command]
pub fn set_note_color(path: String, color: Option<String>, state: State<AppState>) -> Res<()> {
    with_ws_mut(&state, |ws| {
        ws.set_note_color(&path, color.as_deref())
            .map_err(|e| format!("{path}: {e}"))?;
        Ok(())
    })
}

/// Nastaví barvu složky. `None` barvu zruší.
#[tauri::command]
pub fn set_folder_color(path: String, color: Option<String>, state: State<AppState>) -> Res<()> {
    with_ws_mut(&state, |ws| {
        ws.set_folder_color(&path, color.as_deref())
            .map_err(|e| format!("{path}: {e}"))?;
        Ok(())
    })
}

/// Kam vede kliknutí na `[[odkaz]]`. `raw` je obsah mezi závorkami.
#[tauri::command]
pub fn follow_link(from_path: Option<String>, raw: String, state: State<AppState>) -> Res<FollowResult> {
    with_ws(&state, |ws| {
        let source = format!("[[{raw}]]");
        let parsed = parse(&source);
        let link = parsed.links.first().ok_or("odkaz nelze rozparsovat")?;
        let from = from_path.as_deref().and_then(|p| ws.index_of_path(p));
        let resolution = ws.resolve(link, from);

        let fallback_title = if link.target.is_empty() {
            link.display_text()
        } else {
            link.target.rsplit('/').next().unwrap_or(&link.target).to_string()
        };

        Ok(FollowResult::from_resolution(
            ws,
            resolution,
            link.anchor.as_ref(),
            &fallback_title,
        ))
    })
}

#[tauri::command]
pub fn backlinks(path: String, state: State<AppState>) -> Res<Vec<BacklinkEntry>> {
    with_ws(&state, |ws| {
        let Some(index) = ws.index_of_path(&path) else {
            return Ok(Vec::new());
        };
        Ok(ws
            .backlinks(index)
            .into_iter()
            .filter_map(|b| {
                let note = ws.note(b.from)?;
                Some(BacklinkEntry {
                    path: note.path.clone(),
                    title: note.title.clone(),
                    line: b.line,
                    context: b.context,
                })
            })
            .collect())
    })
}

#[tauri::command]
pub fn search(query: String, state: State<AppState>) -> Res<Vec<SearchResult>> {
    with_ws(&state, |ws| {
        // Nálezů z jedné poznámky bývá víc, takže se mapa offsetů jejího obsahu
        // staví jen jednou — jinak by se dlouhá poznámka přemapovala 200×.
        let mut maps: HashMap<String, Utf16Map> = HashMap::new();

        Ok(ws
            .search(&query, 200)
            .into_iter()
            .map(|hit| {
                let note = ws.index_of_path(&hit.path).and_then(|i| ws.note(i));
                let title = note
                    .map(|n| n.title.clone())
                    .unwrap_or_else(|| hit.path.clone());
                let map = maps
                    .entry(hit.path.clone())
                    .or_insert_with(|| Utf16Map::new(note.map_or("", |n| n.content.as_str())));
                SearchResult::from(hit, title, map)
            })
            .collect())
    })
}

#[tauri::command]
pub fn notes_with_tag(tag: String, state: State<AppState>) -> Res<Vec<NoteSummary>> {
    with_ws(&state, |ws| {
        Ok(ws
            .notes_with_tag(&tag)
            .into_iter()
            .filter_map(|i| ws.note(i))
            .map(NoteSummary::from)
            .collect())
    })
}

#[tauri::command]
pub fn all_tags(state: State<AppState>) -> Res<Vec<TagCount>> {
    with_ws(&state, |ws| {
        Ok(ws
            .tag_counts()
            .into_iter()
            .map(|(tag, count)| TagCount { tag, count })
            .collect())
    })
}

/// Odkazy, které zatím nikam nevedou — přehled chybějících poznámek.
#[tauri::command]
pub fn missing_notes(state: State<AppState>) -> Res<Vec<String>> {
    with_ws(&state, |ws| {
        let mut out: Vec<String> = Vec::new();
        for (i, link) in ws.unresolved_links() {
            if let Resolution::Unresolved { suggested_path } = ws.resolve(link, Some(i)) {
                if !out.contains(&suggested_path) {
                    out.push(suggested_path);
                }
            }
        }
        out.sort();
        Ok(out)
    })
}
