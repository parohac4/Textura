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

async function loadWorkspace(path) {
  try {
    state.workspace = await api.openWorkspace(path);
    localStorage.setItem("textura:workspace", path);
    $("status").textContent = `${state.workspace.root} — ${state.workspace.noteCount} poznámek`;
    await Promise.all([loadNotes(), loadTags()]);
  } catch (err) {
    showError(err);
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
  row.onclick = () => openNote(note.path);
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
  for (const folder of state.folders ?? []) descend(folder);
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

async function openNote(path, line = null) {
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
    if (line) goToLine(view, line);
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

function renderSearchHits(list, hits) {
  if (!hits.length) {
    list.append(empty("Nic nenalezeno."));
    return;
  }
  for (const hit of hits) {
    const row = document.createElement("button");
    row.className = "row";
    row.innerHTML = `<span class="row-title"></span><span class="row-sub mono"></span>`;
    row.querySelector(".row-title").textContent = `${hit.title}:${hit.line}`;
    row.querySelector(".row-sub").textContent = hit.snippet;
    row.onclick = () => openNote(hit.path, hit.line);
    list.append(row);
  }
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

/** Složky, které ve workspace existují — včetně prázdných. */
function existingFolders() {
  return [...(state.folders ?? [])].sort((a, b) => a.localeCompare(b, "cs"));
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
      const exists = path && (state.folders ?? []).some((f) => f === path);
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
        // Nová poznámka má být hned vidět i ve stromu.
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
$("new-folder").onclick = () => showNewFolder();
$("settings").onclick = showSettings;
$("reload").onclick = reload;
$("modal-cancel").onclick = closeModal;
$("filter-clear").onclick = clearFilter;
$("modal").onclick = (event) => {
  if (event.target.id === "modal") closeModal();
};
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    // Escape zavírá dialog; když žádný není, ruší filtr v seznamu.
    if (!$("modal").hidden) closeModal();
    else if (state.view.mode !== "tree") clearFilter();
  }
  if ((event.ctrlKey || event.metaKey) && event.key === "n") {
    event.preventDefault();
    showNewNote();
  }
});
window.addEventListener("beforeunload", () => {
  // Poslední šance uložit rozepsanou poznámku.
  if (state.current && state.dirty) api.saveNote(state.current.path, view.state.doc.toString());
});

// Při startu: složka z příkazové řádky, jinak ta naposledy otevřená.
(async () => {
  await applySpellCheck();
  const fromArgs = await api.initialWorkspace().catch(() => null);
  const path = fromArgs || localStorage.getItem("textura:workspace");
  if (path) await loadWorkspace(path);
})();
