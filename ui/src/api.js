// Tenká vrstva nad Tauri commandy. Veškerá znalost formátu je v Rustu —
// frontend si nikdy neparsuje poznámku sám.
import { invoke } from "@tauri-apps/api/core";

export const initialWorkspace = () => invoke("initial_workspace");
export const openWorkspace = (path) => invoke("open_workspace", { path });
export const reloadWorkspace = () => invoke("reload_workspace");
export const listNotes = () => invoke("list_notes");
export const readNote = (path) => invoke("read_note", { path });
export const analyze = (path, content) => invoke("analyze", { path, content });
export const saveNote = (path, content) => invoke("save_note", { path, content });
/**
 * Poskládá text bloku `@mapa` z buněk `{ depth, text, id }` a ručních spojů
 * `{ from, to }`; formát zná jen jádro.
 */
export const renderMindmap = (title, nodes, edges) =>
  invoke("render_mindmap", { title, nodes, edges });
/** Vyrobí stabilní ID buňky, aby šla spojit a zapamatovat si pozici. */
export const mindmapNodeId = (text, taken) => invoke("mindmap_node_id", { text, taken });
/** Uloží pozici odtažené bubliny; `null` ji zruší. */
export const setMindmapPosition = (path, id, x, y) =>
  invoke("set_mindmap_position", { path, id, x, y });
/** Zapomene pozice všech bublin poznámky — mapa se srovná sama. */
export const clearMindmapLayout = (path) => invoke("clear_mindmap_layout", { path });
export const createNote = (path, title) => invoke("create_note", { path, title });
export const listFolders = () => invoke("list_folders");
export const createFolder = (path) => invoke("create_folder", { path });
export const deleteNote = (path) => invoke("delete_note", { path });
export const deleteFolder = (path, recursive) => invoke("delete_folder", { path, recursive });
export const countNotesInFolder = (folder) => invoke("count_notes_in_folder", { folder });
export const moveNote = (from, to) => invoke("move_note", { from, to });
export const moveFolder = (from, to) => invoke("move_folder", { from, to });
export const availableColors = () => invoke("available_colors");
export const setNoteColor = (path, color) => invoke("set_note_color", { path, color });
export const setFolderColor = (path, color) => invoke("set_folder_color", { path, color });
export const followLink = (fromPath, raw) => invoke("follow_link", { fromPath, raw });
export const backlinks = (path) => invoke("backlinks", { path });
export const search = (query) => invoke("search", { query });
export const notesWithTag = (tag) => invoke("notes_with_tag", { tag });
export const allTags = () => invoke("all_tags");
export const missingNotes = () => invoke("missing_notes");
export const availableDictionaries = () => invoke("available_dictionaries");
export const setSpellCheck = (enabled, languages) =>
  invoke("set_spell_check", { enabled, languages });
