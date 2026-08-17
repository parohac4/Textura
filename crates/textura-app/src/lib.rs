//! Desktopová aplikace Textura.
//!
//! Backend drží otevřený workspace a veškerou znalost formátu — UI si samo nic
//! neparsuje, jen vykresluje to, co dostane z [`commands::analyze`]. Díky tomu
//! je pravidlo formátu na jednom místě, v `textura-core`.

pub mod commands;
pub mod dto;
pub mod offsets;
pub mod spellcheck;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            initial: std::env::args().nth(1),
            ..AppState::default()
        })
        .invoke_handler(tauri::generate_handler![
            commands::initial_workspace,
            commands::open_workspace,
            commands::reload_workspace,
            commands::list_notes,
            commands::read_note,
            commands::analyze,
            commands::save_note,
            commands::render_mindmap,
            commands::mindmap_node_id,
            commands::set_mindmap_position,
            commands::clear_mindmap_layout,
            commands::create_note,
            commands::list_folders,
            commands::create_folder,
            commands::delete_note,
            commands::delete_folder,
            commands::count_notes_in_folder,
            commands::move_note,
            commands::move_folder,
            commands::available_colors,
            commands::set_note_color,
            commands::set_folder_color,
            commands::available_dictionaries,
            commands::set_spell_check,
            commands::follow_link,
            commands::backlinks,
            commands::search,
            commands::notes_with_tag,
            commands::all_tags,
            commands::missing_notes,
        ])
        .run(tauri::generate_context!())
        .expect("chyba při startu aplikace");
}
