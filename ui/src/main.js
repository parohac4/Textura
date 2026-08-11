import "./style.css";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, openPath } from "@tauri-apps/plugin-opener";

import * as api from "./api.js";
import { createEditor, setAnalysis, setContent, goToLine } from "./editor.js";
import { loadSettings, saveSettings, applyTheme } from "./settings.js";

const $ = (id) => document.getElementById(id);

const state = {
  workspace: null,
  notes: [],
  current: null, // { path, title }
  dirty: false,
  saveTimer: null,
  analyzeTimer: null,
  searchTimer: null,
  settings: loadSettings(),
  /** Co je zrovna vlevo: strom složek, plochý seznam nebo výsledky hledání. */
  view: { mode: "tree" },
  /** Rozbalené složky (plné cesty). */
  expanded: new Set(JSON.parse(localStorage.getItem("textura:expanded") ?? "[]")),
};

applyTheme(state.settings.theme);

// --- kontrola překlepů -----------------------------------------------------

/** Předá aktuální nastavení kontroly překlepů webview. */
async function applySpellCheck() {
  try {
    state.dictionaries ??= await api.availableDictionaries();

    // Při prvním spuštění zapneme všechny slovníky, které systém nabízí —
    // uživatel obvykle píše česky i anglicky a ruční výběr by ho jen zdržel.
    if (state.settings.spellLanguages === null) {
      state.settings = { ...state.settings, spellLanguages: [...state.dictionaries] };
      saveSettings(state.settings);
    }

    await api.setSpellCheck(state.settings.spellCheck, state.settings.spellLanguages);
  } catch (err) {
    showError(err);
  }
}

const SAVE_DELAY = 600;
const ANALYZE_DELAY = 180;

// --- editor ----------------------------------------------------------------

const view = createEditor($("editor"), {
  onChange: (content) => {
    if (!state.current) return;
    markDirty();
    schedule("saveTimer", SAVE_DELAY, () => save(content));
    schedule("analyzeTimer", ANALYZE_DELAY, () => refreshAnalysis(content));
  },
  onSave: () => {
    if (state.current) save(view.state.doc.toString());
  },
  onActivate: (marker) => {
    if (marker.type === "link") followLink(marker.link);
    else if (marker.type === "url") openUrl(marker.url.text).catch(showError);
    else if (marker.type === "task") toggleTask(marker.task);
  },
});

function schedule(key, delay, fn) {
  clearTimeout(state[key]);
  state[key] = setTimeout(fn, delay);
}

async function refreshAnalysis(content) {
  if (!state.current) return;
  try {
    const analysis = await api.analyze(state.current.path, content);
    view.dispatch({ effects: setAnalysis.of(analysis) });
  } catch (err) {
    showError(err);
  }
}

// --- ukládání --------------------------------------------------------------

function markDirty() {
  state.dirty = true;
  $("save-state").textContent = "neuloženo";
  $("save-state").className = "dirty";
}

async function save(content) {
  if (!state.current) return;
  try {
    await api.saveNote(state.current.path, content);
    state.dirty = false;
    $("save-state").textContent = "uloženo";
    $("save-state").className = "saved";
    // Uložením se mohly změnit odkazy → zpětné odkazy i seznam poznámek.
    await Promise.all([loadNotes(), loadBacklinks(), loadTags()]);
  } catch (err) {
    showError(err);
  }
}

/** Uloží rozepsanou poznámku dřív, než se přepne na jinou. */
async function flushPending() {
  clearTimeout(state.saveTimer);
  if (state.current && state.dirty) await save(view.state.doc.toString());
}

// --- workspace -------------------------------------------------------------

async function chooseWorkspace() {
  const path = await openDialog({
    directory: true,
    multiple: false,
    title: "Vyber složku s poznámkami",
  });
  if (!path) return;
  await loadWorkspace(path);
}

/**
 * `remembered` říká, že cesta nepřišla od uživatele, ale z minulého spuštění.
 * Taková složka se mohla mezitím smazat nebo odejít s odpojeným diskem —
 * vítat uživatele chybou, o kterou si teď neřekl, nemá smysl, tak se jen
 * zapomene.
 */
async function loadWorkspace(path, { remembered = false } = {}) {
  try {
    state.workspace = await api.openWorkspace(path);
    localStorage.setItem("textura:workspace", path);
    $("status").textContent = `${state.workspace.root} — ${state.workspace.noteCount} poznámek`;
    await Promise.all([loadNotes(), loadTags()]);
  } catch (err) {
    if (!remembered) {
      showError(err);
      return;
    }
    localStorage.removeItem("textura:workspace");
    $("status").textContent = `${path} už neexistuje — otevři složku s poznámkami.`;
  }
}

async function reload() {
  if (!state.workspace) return;
  await flushPending();
  try {
    state.workspace = await api.reloadWorkspace();
    await Promise.all([loadNotes(), loadTags(), loadBacklinks()]);
    if (state.current) await openNote(state.current.path);
  } catch (err) {
    showError(err);
  }
}

// --- seznam poznámek -------------------------------------------------------

async function loadNotes() {
  if (!state.workspace) return;
  [state.notes, state.folders] = await Promise.all([api.listNotes(), api.listFolders()]);
  renderView();
}

/** Překreslí levý panel podle aktuálního režimu. */
function renderView() {
  const list = $("list");
  list.replaceChildren();

  if (state.view.mode === "search") {
    renderSearchHits(list, state.view.hits);
  } else if (state.view.mode === "list") {
    renderFlatList(list, state.view.notes);
  } else {
    renderTree(list, state.notes);
  }

  renderFilterBar();
  renderTagChips();
}

/** Lišta s aktivním filtrem — bez ní není ze seznamu cesta zpátky na strom. */
function renderFilterBar() {
  const bar = $("filter");
  const label = $("filter-label");

  if (state.view.mode === "list") {
    label.textContent = `${state.view.label} — ${state.view.notes.length} poznámek`;
    bar.hidden = false;
  } else if (state.view.mode === "search") {
    label.textContent = `hledání — ${state.view.hits.length} nálezů`;
    bar.hidden = false;
  } else {
    bar.hidden = true;
  }
}

/** Zpět na strom všech poznámek. */
function clearFilter() {
  $("search").value = "";
  state.view = { mode: "tree" };
  renderView();
}

function noteRow(note) {
  const row = document.createElement("button");
  row.className = "row" + (state.current?.path === note.path ? " active" : "");
  row.innerHTML = `<span class="row-title"></span><span class="row-sub"></span>`;
  row.querySelector(".row-title").textContent = note.title;
  if (note.color) row.dataset.color = note.color;
  row.onclick = () => openNote(note.path);
  row.oncontextmenu = (event) =>
    showContextMenu(event, note.title, [
      ["Přejmenovat…", () => showRenameNote(note)],
      ["Přesunout…", () => showMoveNote(note)],
      ["Barva…", () => showNoteColor(note)],
      ["Smazat…", () => showDeleteNote(note)],
    ]);
  return row;
}

function renderFlatList(list, notes) {
  if (!notes.length) {
    list.append(empty("Žádné poznámky."));
    return;
  }
  for (const note of notes) {
    const row = noteRow(note);
    row.querySelector(".row-sub").textContent = note.folder || "";
    list.append(row);
  }
}

/** Poskládá strom ze složek workspace a poznámek. */
function buildTree(notes) {
  const root = { folders: new Map(), notes: [] };

  const descend = (folder) => {
    let node = root;
    if (!folder) return node;
    let path = "";
    for (const segment of folder.split("/")) {
      path = path ? `${path}/${segment}` : segment;
      if (!node.folders.has(segment)) {
        node.folders.set(segment, { folders: new Map(), notes: [], path });
      }
      node = node.folders.get(segment);
    }
    return node;
  };

  // Nejdřív složky z workspace, aby se ve stromu objevily i ty prázdné.
  for (const folder of state.folders ?? []) descend(folder.path);
  for (const note of notes) descend(note.folder).notes.push(note);

  return root;
}

function renderTree(list, notes) {
  if (!notes.length) {
    list.append(empty("Žádné poznámky."));
    return;
  }
  renderTreeNode(list, buildTree(notes), 0);
}

function renderTreeNode(parent, node, depth) {
  const folders = [...node.folders.entries()].sort(([a], [b]) =>
    a.localeCompare(b, "cs"),
  );

  for (const [name, child] of folders) {
    const open = state.expanded.has(child.path);

    const row = document.createElement("div");
    row.className = "folder-row";
    row.style.paddingLeft = `${11 + depth * 12}px`;

    const toggle = document.createElement("button");
    toggle.className = "folder";
    toggle.innerHTML = `<span class="caret"></span><span class="folder-name"></span>`;
    toggle.querySelector(".caret").textContent = open ? "▾" : "▸";
    toggle.querySelector(".folder-name").textContent = name;
    toggle.onclick = () => {
      if (open) state.expanded.delete(child.path);
      else state.expanded.add(child.path);
      localStorage.setItem("textura:expanded", JSON.stringify([...state.expanded]));
      renderView();
    };

    const add = document.createElement("button");
    add.className = "folder-add";
    add.textContent = "+";
    add.title = `Nová poznámka ve složce ${child.path}`;
    add.onclick = (event) => {
      event.stopPropagation();
      showNewNote(child.path);
    };

    const color = folderColor(child.path);
    if (color) row.dataset.color = color;

    row.oncontextmenu = (event) =>
      showContextMenu(event, child.path, [
        ["Nová poznámka…", () => showNewNote(child.path)],
        ["Nová poznámka s datem", () => createDatedNote(child.path)],
        ["Nová podsložka…", () => showNewFolder(child.path)],
        ["Přejmenovat…", () => showRenameFolder(child.path)],
        ["Přesunout…", () => showMoveFolder(child.path)],
        ["Barva…", () => showFolderColor(child.path)],
        ["Smazat…", () => showDeleteFolder(child.path)],
      ]);

    row.append(toggle, add);
    parent.append(row);

    if (open) renderTreeNode(parent, child, depth + 1);
  }

  for (const note of node.notes) {
    const row = noteRow(note);
    row.style.paddingLeft = `${11 + depth * 12}px`;
    parent.append(row);
  }
}

// --- poznámka --------------------------------------------------------------

/** `match` je UTF-16 rozsah nálezu, který se má v editoru označit. */
async function openNote(path, line = null, match = null) {
  await flushPending();
  try {
    const note = await api.readNote(path);
    state.current = { path: note.path, title: note.title };
    state.dirty = false;

    $("note-title").textContent = note.title;
    $("note-path").textContent = note.path;
    $("save-state").textContent = "";
    $("save-state").className = "";

    setContent(view, note.content);
    view.dispatch({ effects: setAnalysis.of(note.analysis) });
    if (line) goToLine(view, line, match);
    else view.focus();

    renderView();
    await loadBacklinks();
  } catch (err) {
    showError(err);
  }
}

// --- odkazy ----------------------------------------------------------------

async function followLink(link) {
  try {
    const result = await api.followLink(state.current?.path ?? null, link.raw);
    switch (result.kind) {
      case "note":
        await openNote(result.path, result.line);
        break;
      case "attachment":
        await openPath(result.path);
        break;
      case "ambiguous":
        showPicker(result.candidates);
        break;
      case "unresolved":
        showCreate(result.suggestedPath, result.title);
        break;
    }
  } catch (err) {
    showError(err);
  }
}

function toggleTask(task) {
  // `[ ]` ↔ `[x]`; důležitý úkol `[!]` zůstává důležitý dokud se nesplní.
  const next = task.state === "done" ? " " : "x";
  view.dispatch({
    changes: { from: task.markerFrom + 1, to: task.markerTo - 1, insert: next },
  });
}

// --- backlinky -------------------------------------------------------------

async function loadBacklinks() {
  const pane = $("backlinks");
  pane.replaceChildren();
  if (!state.current) return;

  const links = await api.backlinks(state.current.path);
  if (!links.length) {
    pane.append(empty("Zatím sem nic neodkazuje."));
    return;
  }
  for (const link of links) {
    const row = document.createElement("button");
    row.className = "row";
    row.innerHTML = `<span class="row-title"></span><span class="row-sub mono"></span>`;
    row.querySelector(".row-title").textContent = link.title;
    row.querySelector(".row-sub").textContent = link.context;
    row.onclick = () => openNote(link.path, link.line);
    pane.append(row);
  }
}

// --- hledání ---------------------------------------------------------------

$("search").addEventListener("input", (event) => {
  const query = event.target.value;
  schedule("searchTimer", 200, () => runSearch(query));
});

/** Ctrl+F — kurzor do hledání, s předvybraným dotazem k přepsání. */
function focusSearch() {
  const input = $("search");
  input.focus();
  input.select();
}

async function runSearch(query) {
  if (!state.workspace) return;
  if (!query.trim()) {
    // Vyprázdněné hledání vrací strom, ale filtr podle tagu nechává být.
    if (state.view.mode === "search") state.view = { mode: "tree" };
    renderView();
    return;
  }
  try {
    state.view = { mode: "search", hits: await api.search(query) };
    renderView();
  } catch (err) {
    showError(err);
  }
}

/** Popisek druhu nálezu; nález v těle se pozná už podle čísla řádku. */
const HIT_KINDS = { title: "název", alias: "alias" };

function renderSearchHits(list, hits) {
  if (!hits.length) {
    list.append(empty("Nic nenalezeno."));
    return;
  }
  for (const hit of hits) {
    const row = document.createElement("button");
    row.className = "row";
    row.innerHTML = `<span class="row-title"></span><span class="row-sub mono"></span>`;

    const title = row.querySelector(".row-title");
    title.textContent = hit.kind === "body" ? `${hit.title}:${hit.line}` : hit.title;
    if (HIT_KINDS[hit.kind]) {
      const badge = document.createElement("span");
      badge.className = "hit-kind";
      badge.textContent = HIT_KINDS[hit.kind];
      title.append(badge);
    }

    row.querySelector(".row-sub").append(highlighted(hit.snippet, hit.snippetFrom, hit.snippetTo));
    const match =
      hit.matchFrom === null ? null : { from: hit.matchFrom, to: hit.matchTo };
    row.onclick = () => openNote(hit.path, hit.line, match);
    list.append(row);
  }
}

/**
 * Text se zvýrazněným úsekem `from`–`to` (UTF-16 offsety ze backendu).
 *
 * Skládá se z textových uzlů, ne z HTML — obsah poznámky se do stránky nikdy
 * nevkládá jako značky.
 */
function highlighted(text, from, to) {
  const out = document.createDocumentFragment();
  const start = Math.max(0, Math.min(from ?? 0, text.length));
  const end = Math.max(start, Math.min(to ?? 0, text.length));
  if (end === start) {
    out.append(text);
    return out;
  }

  const mark = document.createElement("mark");
  mark.textContent = text.slice(start, end);
  out.append(text.slice(0, start), mark, text.slice(end));
  return out;
}

// --- tagy ------------------------------------------------------------------

async function loadTags() {
  if (!state.workspace) return;
  state.tags = await api.allTags();
  renderTagChips();
}

function renderTagChips() {
  const pane = $("tags");
  pane.replaceChildren();

  const activeTag = state.view.mode === "list" ? state.view.tag : null;

  for (const { tag, count } of state.tags ?? []) {
    const chip = document.createElement("button");
    chip.className = "chip" + (tag === activeTag ? " active" : "");
    chip.textContent = `#${tag}`;
    const badge = document.createElement("span");
    badge.className = "badge";
    badge.textContent = count;
    chip.append(badge);
    chip.onclick = () => filterByTag(tag);
    pane.append(chip);
  }
}

/** Kliknutí na tag filtruje; kliknutí na už aktivní tag filtr zruší. */
async function filterByTag(tag) {
  if (state.view.mode === "list" && state.view.tag === tag) {
    clearFilter();
    return;
  }
  try {
    $("search").value = "";
    state.view = {
      mode: "list",
      tag,
      label: `#${tag}`,
      notes: await api.notesWithTag(tag),
    };
    renderView();
  } catch (err) {
    showError(err);
  }
}

// --- modal -----------------------------------------------------------------

function showModal(title, build) {
  $("modal-title").textContent = title;
  const body = $("modal-body");
  body.replaceChildren();
  build(body);
  $("modal").hidden = false;
}

function closeModal() {
  $("modal").hidden = true;
}

function showPicker(candidates) {
  showModal("Odkaz je nejednoznačný — vyber poznámku", (body) => {
    for (const note of candidates) {
      const row = document.createElement("button");
      row.className = "row";
      row.innerHTML = `<span class="row-title"></span><span class="row-sub mono"></span>`;
      row.querySelector(".row-title").textContent = note.title;
      row.querySelector(".row-sub").textContent = note.path;
      row.onclick = () => {
        closeModal();
        openNote(note.path);
      };
      body.append(row);
    }
  });
}

// --- kontextové menu -------------------------------------------------------

/** Menu na pravý klik. `items` je pole dvojic [popisek, akce]. */
function showContextMenu(event, heading, items) {
  event.preventDefault();
  event.stopPropagation();

  const menu = $("context-menu");
  menu.replaceChildren();

  const title = document.createElement("li");
  title.className = "menu-heading";
  title.textContent = heading;
  menu.append(title);

  for (const [label, action] of items) {
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.textContent = label;
    button.onclick = () => {
      closeContextMenu();
      action();
    };
    item.append(button);
    menu.append(item);
  }

  // Nejdřív zobrazit, ať se dá změřit, pak teprve umístit do okna.
  menu.hidden = false;
  const { offsetWidth: w, offsetHeight: h } = menu;
  menu.style.left = `${Math.min(event.clientX, window.innerWidth - w - 8)}px`;
  menu.style.top = `${Math.min(event.clientY, window.innerHeight - h - 8)}px`;
}

function closeContextMenu() {
  $("context-menu").hidden = true;
}

// --- přesun a mazání -------------------------------------------------------

/** Nabídka cílových složek pro `<select>`. */
function folderOptions(select, { exclude = [] } = {}) {
  select.append(new Option("(kořen workspace)", ""));
  for (const folder of existingFolders()) {
    if (exclude.some((e) => folder === e || folder.startsWith(`${e}/`))) continue;
    select.append(new Option(folder, folder));
  }
}

/**
 * Dialog s jedním textovým polem.
 *
 * @param onSubmit dostane zadanou hodnotu
 * @param validate vrátí chybovou hlášku, nebo null když je hodnota v pořádku
 */
function showNameDialog({ title, label, value, hint, submitLabel, validate, onSubmit }) {
  showModal(title, (body) => {
    if (hint) {
      const info = document.createElement("p");
      info.textContent = hint;
      body.append(info);
    }

    const field = document.createElement("label");
    field.textContent = label;
    const input = document.createElement("input");
    input.type = "text";
    input.value = value;
    field.append(input);

    const preview = document.createElement("code");
    preview.className = "block";

    const confirm = document.createElement("button");
    confirm.className = "primary";
    confirm.textContent = submitLabel;

    function update() {
      const problem = validate(input.value.trim());
      preview.textContent = problem ?? input.value.trim() ?? "…";
      preview.className = problem ? "block conflict" : "block";
      confirm.disabled = Boolean(problem) || !input.value.trim();
    }

    confirm.onclick = () => {
      const next = input.value.trim();
      closeModal();
      onSubmit(next);
    };
    input.oninput = update;
    input.onkeydown = (event) => {
      if (event.key === "Enter" && !confirm.disabled) confirm.click();
    };

    body.append(field, preview, confirm);
    update();
    setTimeout(() => {
      input.focus();
      input.select();
    }, 0);
  });
}

function showRenameNote(note) {
  showNameDialog({
    title: "Přejmenovat poznámku",
    label: "Nový název",
    value: note.name,
    hint: note.path,
    submitLabel: "Přejmenovat",
    validate: (next) => {
      const base = next.replace(/\.txt$/i, "");
      if (!base) return null;
      if (base === note.name) return null;
      if (base.includes("/")) return "název nesmí obsahovat lomítko";
      const target = note.folder ? `${note.folder}/${base}.txt` : `${base}.txt`;
      return state.notes.some((n) => n.path.toLowerCase() === target.toLowerCase())
        ? `${target} — už existuje`
        : null;
    },
    onSubmit: async (next) => {
      const base = next.replace(/\.txt$/i, "");
      if (base === note.name) return;
      const target = note.folder ? `${note.folder}/${base}.txt` : `${base}.txt`;
      try {
        const outcome = await api.moveNote(note.path, target);
        if (state.current?.path === note.path) state.current.path = outcome.path;
        await loadNotes();
        await loadTags();
        if (state.current?.path === outcome.path) await openNote(outcome.path);
        reportMove(outcome);
      } catch (err) {
        showError(err);
      }
    },
  });
}

function showRenameFolder(path) {
  const parent = path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "";
  const name = path.split("/").pop();

  showNameDialog({
    title: "Přejmenovat složku",
    label: "Nový název",
    value: name,
    hint: path,
    submitLabel: "Přejmenovat",
    validate: (next) => {
      if (!next || next === name) return null;
      if (next.includes("/")) return "název nesmí obsahovat lomítko";
      const target = parent ? `${parent}/${next}` : next;
      return (state.folders ?? []).some((f) => f.path === target)
        ? `${target} — už existuje`
        : null;
    },
    onSubmit: async (next) => {
      if (next === name) return;
      const target = parent ? `${parent}/${next}` : next;
      try {
        const outcome = await api.moveFolder(path, target);
        remapExpanded(path, target);
        const current = state.current?.path;
        if (current?.startsWith(`${path}/`)) {
          state.current.path = target + current.slice(path.length);
        }
        await loadNotes();
        await loadTags();
        if (state.current) await openNote(state.current.path);
        reportMove(outcome);
      } catch (err) {
        showError(err);
      }
    },
  });
}

/** Rozbalený stav složek se posune spolu s přejmenovanou či přesunutou složkou. */
function remapExpanded(from, to) {
  for (const open of [...state.expanded]) {
    if (open === from || open.startsWith(`${from}/`)) {
      state.expanded.delete(open);
      state.expanded.add(to + open.slice(from.length));
    }
  }
  state.expanded.add(to);
  localStorage.setItem("textura:expanded", JSON.stringify([...state.expanded]));
}

// --- barvy -----------------------------------------------------------------

/** Dialog s paletou. `current` je aktuální barva, `onPick` dostane novou nebo null. */
function showColorPicker(title, hint, current, onPick) {
  showModal(title, (body) => {
    const info = document.createElement("p");
    info.textContent = hint;

    const palette = document.createElement("div");
    palette.className = "palette";

    for (const color of state.colors ?? []) {
      const swatch = document.createElement("button");
      swatch.className = "swatch" + (color === current ? " active" : "");
      swatch.dataset.color = color;
      swatch.title = color;
      swatch.onclick = () => {
        closeModal();
        onPick(color);
      };
      palette.append(swatch);
    }

    const clear = document.createElement("button");
    clear.className = "ghost";
    clear.textContent = "Bez barvy";
    clear.onclick = () => {
      closeModal();
      onPick(null);
    };

    body.append(info, palette, clear);
  });
}

function showNoteColor(note) {
  showColorPicker(
    "Barva poznámky",
    "Uloží se do poznámky jako @color, takže ji uvidíš i v obyčejném textovém editoru.",
    note.color ?? null,
    async (color) => {
      try {
        await api.setNoteColor(note.path, color);
        await loadNotes();
        // Barva je property, takže se změnil i obsah otevřené poznámky.
        if (state.current?.path === note.path) await openNote(note.path);
      } catch (err) {
        showError(err);
      }
    },
  );
}

function showFolderColor(path) {
  showColorPicker(
    "Barva složky",
    "Složka nemá vlastní soubor, barva se proto ukládá do .txtnote/colors.txt.",
    folderColor(path),
    async (color) => {
      try {
        await api.setFolderColor(path, color);
        state.folders = await api.listFolders();
        renderView();
      } catch (err) {
        showError(err);
      }
    },
  );
}

function showMoveNote(note) {
  showModal(`Přesunout poznámku`, (body) => {
    const info = document.createElement("p");
    info.textContent = note.path;

    const folderLabel = document.createElement("label");
    folderLabel.textContent = "Cílová složka";
    const folder = document.createElement("select");
    folderOptions(folder);
    folder.value = note.folder;
    folderLabel.append(folder);

    const nameLabel = document.createElement("label");
    nameLabel.textContent = "Název";
    const name = document.createElement("input");
    name.type = "text";
    name.value = note.name;
    nameLabel.append(name);

    const preview = document.createElement("code");
    preview.className = "block";
    const warning = document.createElement("p");
    warning.className = "warn";

    const confirm = document.createElement("button");
    confirm.className = "primary";
    confirm.textContent = "Přesunout";

    const targetPath = () => {
      const base = name.value.trim().replace(/\.txt$/i, "");
      if (!base) return "";
      return folder.value ? `${folder.value}/${base}.txt` : `${base}.txt`;
    };

    function update() {
      const path = targetPath();
      const same = path === note.path;
      const taken =
        path && !same && state.notes.some((n) => n.path.toLowerCase() === path.toLowerCase());

      preview.textContent = taken ? `${path} — už existuje` : path || "…";
      preview.className = taken ? "block conflict" : "block";
      confirm.disabled = !path || same || taken;

      // Odkazy [[Název]] bez cesty se při přejmenování rozbijí.
      warning.textContent =
        name.value.trim().replace(/\.txt$/i, "") !== note.name
          ? "Změna názvu rozbije odkazy, které na poznámku míří jen názvem. Odkazy s celou cestou se opraví."
          : "";
    }

    confirm.onclick = async () => {
      const path = targetPath();
      closeModal();
      try {
        const outcome = await api.moveNote(note.path, path);
        if (state.current?.path === note.path) state.current.path = outcome.path;
        await loadNotes();
        await loadTags();
        if (state.current?.path === outcome.path) await openNote(outcome.path);
        reportMove(outcome);
      } catch (err) {
        showError(err);
      }
    };

    name.oninput = update;
    folder.onchange = update;
    name.onkeydown = (event) => {
      if (event.key === "Enter" && !confirm.disabled) confirm.click();
    };

    body.append(info, folderLabel, nameLabel, preview, warning, confirm);
    update();
    setTimeout(() => name.focus(), 0);
  });
}

function showMoveFolder(path) {
  const name = path.split("/").pop();

  showModal("Přesunout složku", (body) => {
    const info = document.createElement("p");
    info.textContent = path;

    const parentLabel = document.createElement("label");
    parentLabel.textContent = "Nadřazená složka";
    const parent = document.createElement("select");
    // Sebe ani vlastní podsložky nabízet nesmíme — vznikl by cyklus.
    folderOptions(parent, { exclude: [path] });
    parentLabel.append(parent);

    const preview = document.createElement("code");
    preview.className = "block";

    const confirm = document.createElement("button");
    confirm.className = "primary";
    confirm.textContent = "Přesunout";

    const targetPath = () => (parent.value ? `${parent.value}/${name}` : name);

    function update() {
      const target = targetPath();
      const same = target === path;
      const taken = !same && (state.folders ?? []).some((f) => f.path === target);
      preview.textContent = taken ? `${target} — už existuje` : target;
      preview.className = taken ? "block conflict" : "block";
      confirm.disabled = same || taken;
    }

    confirm.onclick = async () => {
      const target = targetPath();
      closeModal();
      try {
        const outcome = await api.moveFolder(path, target);
        remapExpanded(path, target);

        const current = state.current?.path;
        if (current?.startsWith(`${path}/`)) {
          state.current.path = target + current.slice(path.length);
        }
        await loadNotes();
        await loadTags();
        if (state.current) await openNote(state.current.path);
        reportMove(outcome);
      } catch (err) {
        showError(err);
      }
    };

    parent.onchange = update;
    body.append(info, parentLabel, preview, confirm);
    update();
  });
}

function reportMove(outcome) {
  const parts = [];
  if (outcome.updatedLinks) {
    parts.push(
      `opraveno ${outcome.updatedLinks} odkazů v ${outcome.updatedNotes} poznámkách`,
    );
  }
  if (outcome.bareLinks) {
    parts.push(`${outcome.bareLinks} odkazů míří jen názvem — zkontroluj je`);
  }
  $("status").textContent = parts.length
    ? `Přesunuto: ${parts.join("; ")}.`
    : "Přesunuto.";
  $("status").className = "";
}

function showDeleteNote(note) {
  showModal("Smazat poznámku", (body) => {
    const info = document.createElement("p");
    info.textContent = "Přesune se do koše, odkud ji jde vrátit:";

    const path = document.createElement("code");
    path.className = "block";
    path.textContent = note.path;

    const note2 = document.createElement("p");
    note2.textContent = "Koš je běžná složka .txtnote/trash/ ve workspace.";

    const confirm = document.createElement("button");
    confirm.className = "primary danger";
    confirm.textContent = "Smazat";
    confirm.onclick = async () => {
      closeModal();
      try {
        const trashed = await api.deleteNote(note.path);
        if (state.current?.path === note.path) closeNote();
        await loadNotes();
        await loadTags();
        $("status").textContent = `Smazáno, leží v ${trashed}`;
        $("status").className = "";
      } catch (err) {
        showError(err);
      }
    };

    body.append(info, path, note2, confirm);
  });
}

async function showDeleteFolder(path) {
  let count = 0;
  try {
    count = await api.countNotesInFolder(path);
  } catch (err) {
    showError(err);
    return;
  }

  showModal("Smazat složku", (body) => {
    const info = document.createElement("p");
    info.textContent = count
      ? `Složka obsahuje ${count} poznámek. Smažou se všechny:`
      : "Složka je prázdná:";

    const target = document.createElement("code");
    target.className = "block";
    target.textContent = path;

    const hint = document.createElement("p");
    hint.textContent = "Vše se přesune do koše .txtnote/trash/ ve workspace.";

    const confirm = document.createElement("button");
    confirm.className = "primary danger";
    confirm.textContent = count ? `Smazat i s ${count} poznámkami` : "Smazat";
    confirm.onclick = async () => {
      closeModal();
      try {
        const trashed = await api.deleteFolder(path, count > 0);
        if (state.current?.path.startsWith(`${path}/`)) closeNote();
        for (const open of [...state.expanded]) {
          if (open === path || open.startsWith(`${path}/`)) state.expanded.delete(open);
        }
        await loadNotes();
        await loadTags();
        $("status").textContent = `Smazáno, leží v ${trashed}`;
        $("status").className = "";
      } catch (err) {
        showError(err);
      }
    };

    body.append(info, target, hint, confirm);
  });
}

/** Zavře otevřenou poznámku (smazala se nebo zmizela pod rukama). */
function closeNote() {
  state.current = null;
  state.dirty = false;
  clearTimeout(state.saveTimer);
  setContent(view, "");
  $("note-title").textContent = "Textura";
  $("note-path").textContent = "";
  $("save-state").textContent = "";
  $("backlinks").replaceChildren();
}

/** Složky, které ve workspace existují — včetně prázdných. */
function existingFolders() {
  return (state.folders ?? [])
    .map((f) => f.path)
    .sort((a, b) => a.localeCompare(b, "cs"));
}

/** Barva složky, pokud je nastavená. */
function folderColor(path) {
  return (state.folders ?? []).find((f) => f.path === path)?.color ?? null;
}

/** Dialog „nová složka“. `preselect` je nadřazená složka. */
function showNewFolder(preselect = "") {
  if (!state.workspace) {
    showError("nejdřív otevři složku s poznámkami");
    return;
  }

  showModal("Nová složka", (body) => {
    const parentLabel = document.createElement("label");
    parentLabel.textContent = "Nadřazená složka";
    const parent = document.createElement("select");
    parent.append(new Option("(kořen workspace)", ""));
    for (const f of existingFolders()) parent.append(new Option(f, f));
    parent.value = preselect;
    parentLabel.append(parent);

    const nameLabel = document.createElement("label");
    nameLabel.textContent = "Název";
    const name = document.createElement("input");
    name.type = "text";
    name.placeholder = "Například Projekty";
    nameLabel.append(name);

    const preview = document.createElement("code");
    preview.className = "block";

    const confirm = document.createElement("button");
    confirm.className = "primary";
    confirm.textContent = "Vytvořit";

    // Lomítko v názvu vytvoří rovnou několik úrovní najednou.
    const targetPath = () => {
      const base = name.value.trim().replace(/^\/+|\/+$/g, "");
      if (!base) return "";
      return parent.value ? `${parent.value}/${base}` : base;
    };

    function updatePreview() {
      const path = targetPath();
      const exists = path && (state.folders ?? []).some((f) => f.path === path);
      preview.textContent = exists ? `${path} — už existuje` : path || "…";
      preview.className = exists ? "block conflict" : "block";
      confirm.disabled = !path || exists;
    }

    confirm.onclick = async () => {
      const path = targetPath();
      if (!path) return;
      closeModal();
      try {
        state.folders = await api.createFolder(path);
        // Nová složka má být rovnou vidět rozbalená.
        let acc = "";
        for (const segment of path.split("/")) {
          acc = acc ? `${acc}/${segment}` : segment;
          state.expanded.add(acc);
        }
        localStorage.setItem("textura:expanded", JSON.stringify([...state.expanded]));
        clearFilter();
      } catch (err) {
        showError(err);
      }
    };

    name.oninput = updatePreview;
    parent.onchange = updatePreview;
    name.onkeydown = (event) => {
      if (event.key === "Enter" && !confirm.disabled) confirm.click();
    };

    body.append(parentLabel, nameLabel, preview, confirm);
    updatePreview();
    setTimeout(() => name.focus(), 0);
  });
}

/** Nová poznámka má být hned vidět ve stromu i v editoru. */
async function revealCreated(created) {
  if (created.folder) {
    let acc = "";
    for (const segment of created.folder.split("/")) {
      acc = acc ? `${acc}/${segment}` : segment;
      state.expanded.add(acc);
    }
    localStorage.setItem("textura:expanded", JSON.stringify([...state.expanded]));
  }
  $("search").value = "";
  state.view = { mode: "tree" };
  await loadNotes();
  await openNote(created.path);
}

/** Složka, ve které se uživatel právě nachází — podle otevřené poznámky. */
function currentFolder() {
  const path = state.current?.path ?? "";
  const cut = path.lastIndexOf("/");
  return cut === -1 ? "" : path.slice(0, cut);
}

/**
 * Poznámka pojmenovaná aktuálním datem a časem, bez ptaní.
 *
 * V názvu souboru je čas s pomlčkami — dvojtečka na některých systémech
 * v názvu souboru nefunguje. V nadpisu uvnitř poznámky zůstává běžný tvar.
 */
async function createDatedNote(folder = currentFolder()) {
  if (!state.workspace) {
    showError("nejdřív otevři složku s poznámkami");
    return;
  }

  const now = new Date();
  const pad = (n) => String(n).padStart(2, "0");
  const date = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
  const time = `${pad(now.getHours())}-${pad(now.getMinutes())}-${pad(now.getSeconds())}`;
  const name = `${date} ${time}`;
  const path = folder ? `${folder}/${name}.txt` : `${name}.txt`;

  try {
    const created = await api.createNote(path, `${date} ${time.replace(/-/g, ":")}`);
    await revealCreated(created);
  } catch (err) {
    showError(err);
  }
}

/** Dialog „nová poznámka“. `preselect` je složka, do které se má založit. */
function showNewNote(preselect = "") {
  if (!state.workspace) {
    showError("nejdřív otevři složku s poznámkami");
    return;
  }

  showModal("Nová poznámka", (body) => {
    const nameLabel = document.createElement("label");
    nameLabel.textContent = "Název";
    const name = document.createElement("input");
    name.type = "text";
    name.placeholder = "Například Kubernetes";
    nameLabel.append(name);

    const folderLabel = document.createElement("label");
    folderLabel.textContent = "Složka";
    const folder = document.createElement("select");
    const root = new Option("(kořen workspace)", "");
    folder.append(root);
    for (const f of existingFolders()) folder.append(new Option(f, f));
    // Sentinel se porovnává přes referenci na prvek, ne přes hodnotu —
    // řetězcová hodnota by mohla kolidovat s názvem skutečné složky.
    const newFolderOption = new Option("Nová složka…", "");
    folder.append(newFolderOption);
    const creatingNewFolder = () => folder.selectedOptions[0] === newFolderOption;
    folder.value = preselect;
    folderLabel.append(folder);

    const newFolderLabel = document.createElement("label");
    newFolderLabel.textContent = "Cesta nové složky";
    newFolderLabel.hidden = true;
    const newFolder = document.createElement("input");
    newFolder.type = "text";
    newFolder.placeholder = "Například Projekty/2026";
    newFolderLabel.append(newFolder);

    folder.onchange = () => {
      newFolderLabel.hidden = !creatingNewFolder();
      if (!newFolderLabel.hidden) newFolder.focus();
      updatePreview();
    };

    const preview = document.createElement("code");
    preview.className = "block";

    const targetFolder = () =>
      creatingNewFolder() ? newFolder.value.trim().replace(/^\/+|\/+$/g, "") : folder.value;

    const targetPath = () => {
      // Název bez .txt; lomítko v názvu funguje jako podsložka.
      const base = name.value.trim().replace(/\.txt$/i, "");
      if (!base) return "";
      const dir = targetFolder();
      return dir ? `${dir}/${base}.txt` : `${base}.txt`;
    };

    const confirm = document.createElement("button");
    confirm.className = "primary";
    confirm.textContent = "Vytvořit";

    function updatePreview() {
      const path = targetPath();
      preview.textContent = path || "…";
      const exists = path && state.notes.some((n) => n.path.toLowerCase() === path.toLowerCase());
      confirm.disabled = !path || exists;
      preview.className = exists ? "block conflict" : "block";
      if (exists) preview.textContent = `${path} — už existuje`;
    }

    confirm.onclick = async () => {
      const path = targetPath();
      if (!path) return;
      closeModal();
      try {
        const created = await api.createNote(path, name.value.trim().replace(/\.txt$/i, ""));
        await revealCreated(created);
      } catch (err) {
        showError(err);
      }
    };

    name.oninput = updatePreview;
    newFolder.oninput = updatePreview;
    name.onkeydown = (event) => {
      if (event.key === "Enter" && !confirm.disabled) confirm.click();
    };

    body.append(nameLabel, folderLabel, newFolderLabel, preview, confirm);
    updatePreview();
    setTimeout(() => name.focus(), 0);
  });
}

/** Přepínač s několika stavy. `onPick` dostane zvolenou hodnotu. */
function segmented(current, options, onPick) {
  const group = document.createElement("div");
  group.className = "segmented";
  for (const [value, text] of options) {
    const button = document.createElement("button");
    button.textContent = text;
    button.className = current === value ? "active" : "";
    button.onclick = () => {
      for (const sibling of group.children) sibling.className = "";
      button.className = "active";
      onPick(value);
    };
    group.append(button);
  }
  return group;
}

function update(patch) {
  state.settings = { ...state.settings, ...patch };
  saveSettings(state.settings);
}

function showSettings() {
  showModal("Nastavení", (body) => {
    const themeLabel = document.createElement("label");
    themeLabel.textContent = "Motiv";
    themeLabel.append(
      segmented(
        state.settings.theme,
        [
          ["system", "Podle systému"],
          ["light", "Světlý"],
          ["dark", "Tmavý"],
        ],
        (value) => {
          update({ theme: value });
          applyTheme(value);
        },
      ),
    );

    const spellLabel = document.createElement("label");
    spellLabel.textContent = "Kontrola překlepů";
    spellLabel.append(
      segmented(
        state.settings.spellCheck ? "on" : "off",
        [
          ["on", "Zapnutá"],
          ["off", "Vypnutá"],
        ],
        (value) => {
          update({ spellCheck: value === "on" });
          applySpellCheck();
          dictionaries.hidden = !state.settings.spellCheck;
        },
      ),
    );

    const dictionaries = document.createElement("label");
    dictionaries.textContent = "Slovníky";
    dictionaries.hidden = !state.settings.spellCheck;

    const list = document.createElement("div");
    list.className = "checklist";

    if (!state.dictionaries?.length) {
      const none = document.createElement("p");
      none.textContent =
        "V systému nejsou žádné slovníky. Na Debianu je doinstaluješ třeba jako hunspell-cs.";
      list.append(none);
    }

    for (const dict of state.dictionaries ?? []) {
      const row = document.createElement("label");
      row.className = "check";
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = (state.settings.spellLanguages ?? []).includes(dict);
      box.onchange = () => {
        const selected = new Set(state.settings.spellLanguages ?? []);
        if (box.checked) selected.add(dict);
        else selected.delete(dict);
        update({ spellLanguages: [...selected].sort() });
        applySpellCheck();
      };
      row.append(box, document.createTextNode(dict));
      list.append(row);
    }
    dictionaries.append(list);

    const note = document.createElement("p");
    note.textContent =
      "Kontrola překlepů používá slovníky nainstalované v systému. Nastavení se ukládá jen v aplikaci, poznámek se nedotýká.";

    body.append(themeLabel, spellLabel, dictionaries, note);
  });
}

function showCreate(suggestedPath, title) {
  showModal("Poznámka zatím neexistuje", (body) => {
    const info = document.createElement("p");
    info.textContent = "Vytvořit ji jako:";
    const path = document.createElement("code");
    path.className = "block";
    path.textContent = suggestedPath;

    const confirm = document.createElement("button");
    confirm.className = "primary";
    confirm.textContent = "Vytvořit poznámku";
    confirm.onclick = async () => {
      closeModal();
      try {
        const note = await api.createNote(suggestedPath, title);
        await loadNotes();
        await openNote(note.path);
      } catch (err) {
        showError(err);
      }
    };

    body.append(info, path, confirm);
  });
}

// --- drobnosti -------------------------------------------------------------

function empty(text) {
  const div = document.createElement("div");
  div.className = "empty";
  div.textContent = text;
  return div;
}

function showError(err) {
  const message = typeof err === "string" ? err : (err?.message ?? String(err));
  $("status").textContent = `Chyba: ${message}`;
  $("status").className = "error";
  setTimeout(() => ($("status").className = ""), 6000);
}

$("open-workspace").onclick = chooseWorkspace;
$("new-note").onclick = () => showNewNote();
$("new-dated-note").onclick = () => createDatedNote();
$("new-folder").onclick = () => showNewFolder();
$("settings").onclick = showSettings;
$("reload").onclick = reload;
$("modal-cancel").onclick = closeModal;
$("filter-clear").onclick = clearFilter;
$("modal").onclick = (event) => {
  if (event.target.id === "modal") closeModal();
};
// Kontextové menu zmizí na kliknutí kamkoli jinam, na kolečko i na Escape.
document.addEventListener("click", closeContextMenu);
document.addEventListener("scroll", closeContextMenu, true);
window.addEventListener("blur", closeContextMenu);
document.addEventListener("contextmenu", (event) => {
  // Pravý klik mimo řádky menu jen zavře; v editoru necháme systémové menu
  // s návrhy oprav překlepů.
  if (!event.target.closest(".row, .folder-row, #context-menu")) closeContextMenu();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    // Escape zavírá postupně: menu, dialog, filtr v seznamu.
    const inSearch = document.activeElement === $("search");
    if (!$("context-menu").hidden) closeContextMenu();
    else if (!$("modal").hidden) closeModal();
    else if (state.view.mode !== "tree" || inSearch) {
      clearFilter();
      // Z hledání se Escapem vrací kurzor do rozepsané poznámky.
      if (inSearch && state.current) view.focus();
    }
  }
  if ((event.ctrlKey || event.metaKey) && !event.shiftKey && event.key.toLowerCase() === "n") {
    event.preventDefault();
    showNewNote();
  }
  if ((event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === "n") {
    event.preventDefault();
    createDatedNote();
  }
  if ((event.ctrlKey || event.metaKey) && event.key === "f") {
    // Bez preventDefault by webview otevřel vlastní hledání v stránce.
    event.preventDefault();
    focusSearch();
  }
});
window.addEventListener("beforeunload", () => {
  // Poslední šance uložit rozepsanou poznámku.
  if (state.current && state.dirty) api.saveNote(state.current.path, view.state.doc.toString());
});

// Při startu: složka z příkazové řádky, jinak ta naposledy otevřená.
(async () => {
  state.colors = await api.availableColors().catch(() => []);
  await applySpellCheck();
  const fromArgs = await api.initialWorkspace().catch(() => null);
  const path = fromArgs || localStorage.getItem("textura:workspace");
  if (path) await loadWorkspace(path, { remembered: !fromArgs });
})();
