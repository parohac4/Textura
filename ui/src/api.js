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
export const createNote = (path, title) => invoke("create_note", { path, title });
export const listFolders = () => invoke("list_folders");
export const createFolder = (path) => invoke("create_folder", { path });
export const followLink = (fromPath, raw) => invoke("follow_link", { fromPath, raw });
export const backlinks = (path) => invoke("backlinks", { path });
export const search = (query) => invoke("search", { query });
export const notesWithTag = (tag) => invoke("notes_with_tag", { tag });
export const allTags = () => invoke("all_tags");
export const missingNotes = () => invoke("missing_notes");
export const availableDictionaries = () => invoke("available_dictionaries");
export const setSpellCheck = (enabled, languages) =>
  invoke("set_spell_check", { enabled, languages });
