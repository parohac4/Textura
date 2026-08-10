// CodeMirror editor pro plain-text poznámky.
//
// Editor nic neparsuje — dekorace dostane hotové z backendu (`analyze`),
// takže pravidla formátu zůstávají na jednom místě v `textura-core`.

import { EditorState, StateEffect, StateField } from "@codemirror/state";
import {
  EditorView,
  Decoration,
  keymap,
  highlightActiveLine,
  drawSelection,
  rectangularSelection,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";

/** Efekt, kterým se do editoru nalije nová analýza z backendu. */
export const setAnalysis = StateEffect.define();

const cls = {
  section: Decoration.mark({ class: "tx-section" }),
  property: Decoration.mark({ class: "tx-property" }),
  tag: Decoration.mark({ class: "tx-tag" }),
  url: Decoration.mark({ class: "tx-url" }),
  comment: Decoration.mark({ class: "tx-comment" }),
  taskDone: Decoration.mark({ class: "tx-task-done" }),
  taskImportant: Decoration.mark({ class: "tx-task-important" }),
  marker: Decoration.mark({ class: "tx-task-marker" }),
};

function linkDecoration(link) {
  const classes = ["tx-link", `tx-link-${link.state}`];
  if (link.kind === "attachment") classes.push("tx-link-attachment");
  if (link.embed) classes.push("tx-link-embed");
  return Decoration.mark({
    class: classes.join(" "),
    attributes: {
      "data-raw": link.raw,
      title:
        link.state === "unresolved"
          ? `${link.display} — poznámka zatím neexistuje, Ctrl+klik ji založí`
          : link.state === "ambiguous"
            ? `${link.display} — víc kandidátů, Ctrl+klik nabídne výběr`
            : `${link.display} — Ctrl+klik otevře`,
    },
  });
}

function buildDecorations(analysis, docLength) {
  if (!analysis) return Decoration.none;
  const ranges = [];
  const push = (from, to, deco) => {
    const a = Math.max(0, Math.min(from, docLength));
    const b = Math.max(0, Math.min(to, docLength));
    if (b > a) ranges.push(deco.range(a, b));
  };

  for (const p of analysis.properties) push(p.from, p.to, cls.property);
  for (const s of analysis.sections) push(s.from, s.to, cls.section);
  for (const t of analysis.tags) push(t.from, t.to, cls.tag);
  for (const u of analysis.urls) push(u.from, u.to, cls.url);
  for (const c of analysis.comments) push(c.from, c.to, cls.comment);
  for (const l of analysis.links) push(l.from, l.to, linkDecoration(l));
  for (const t of analysis.tasks) {
    push(t.markerFrom, t.markerTo, cls.marker);
    if (t.state === "done") push(t.from, t.to, cls.taskDone);
    if (t.state === "important") push(t.from, t.to, cls.taskImportant);
  }

  // `true` = CodeMirror si rozsahy setřídí sám; překryvy jsou v pořádku.
  return Decoration.set(ranges, true);
}

const analysisField = StateField.define({
  create: () => ({ analysis: null, decorations: Decoration.none }),
  update(value, tr) {
    for (const effect of tr.effects) {
      if (effect.is(setAnalysis)) {
        return {
          analysis: effect.value,
          decorations: buildDecorations(effect.value, tr.newDoc.length),
        };
      }
    }
    if (tr.docChanged) {
      // Do příchodu čerstvé analýzy posuneme stávající dekorace s textem,
      // aby zvýraznění při psaní neposkakovalo.
      return {
        analysis: value.analysis,
        decorations: value.decorations.map(tr.changes),
      };
    }
    return value;
  },
  provide: (field) => EditorView.decorations.from(field, (v) => v.decorations),
});

/** Analýza aktuálně vykreslená v editoru. */
export function currentAnalysis(view) {
  return view.state.field(analysisField).analysis;
}

/** Marker (odkaz / checkbox) na dané pozici v dokumentu. */
export function markerAt(view, pos) {
  const analysis = currentAnalysis(view);
  if (!analysis) return null;

  for (const link of analysis.links) {
    if (pos >= link.from && pos <= link.to) return { type: "link", link };
  }
  for (const url of analysis.urls) {
    if (pos >= url.from && pos <= url.to) return { type: "url", url };
  }
  for (const task of analysis.tasks) {
    if (pos >= task.markerFrom && pos <= task.markerTo) return { type: "task", task };
  }
  return null;
}

/** Automatické pokračování seznamů a úkolů na Enter. */
const continueList = {
  key: "Enter",
  run(view) {
    const { state } = view;
    const line = state.doc.lineAt(state.selection.main.head);
    const match = line.text.match(/^(\s*)(-\s+|\d+\.\s+|\[[ xX!]\]\s+)/);
    if (!match) return false;

    const [prefix, indent, marker] = match;
    // Prázdná položka na Enter seznam ukončí, ne prodlouží.
    if (line.text.trim() === marker.trim()) {
      view.dispatch({
        changes: { from: line.from, to: line.to, insert: indent },
        selection: { anchor: line.from + indent.length },
      });
      return true;
    }

    let next = marker;
    const numbered = marker.match(/^(\d+)\.(\s+)$/);
    if (numbered) next = `${Number(numbered[1]) + 1}.${numbered[2]}`;
    // Hotový úkol pokračuje jako nehotový — nový řádek ještě splněný není.
    if (/^\[[xX!]\]/.test(marker)) next = marker.replace(/^\[.\]/, "[ ]");

    const insert = state.lineBreak + indent + next;
    const pos = state.selection.main.head;
    view.dispatch({
      changes: { from: pos, insert },
      selection: { anchor: pos + insert.length },
      scrollIntoView: true,
    });
    return true;
  },
};

const theme = EditorView.theme({
  "&": { height: "100%", fontSize: "15px" },
  ".cm-scroller": {
    fontFamily: "var(--mono)",
    lineHeight: "1.65",
    padding: "18px 22px 40vh 22px",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-content": { caretColor: "var(--accent)" },
  ".cm-line": { padding: "0" },
});

/**
 * Vytvoří editor.
 *
 * @param parent     kam se editor připojí
 * @param handlers   { onChange, onActivate(marker, event), onSave }
 */
export function createEditor(parent, handlers) {
  const clickHandler = EditorView.domEventHandlers({
    mousedown(event, view) {
      const activate = event.ctrlKey || event.metaKey;
      const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
      if (pos == null) return false;
      const marker = markerAt(view, pos);
      if (!marker) return false;

      // Checkbox se překlikává obyčejným klikem, odkazy až s Ctrl —
      // jinak by nešlo do odkazu kliknout kurzorem.
      if (marker.type === "task" && !activate) {
        handlers.onActivate(marker, event);
        event.preventDefault();
        return true;
      }
      if (activate) {
        handlers.onActivate(marker, event);
        event.preventDefault();
        return true;
      }
      return false;
    },
  });

  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: "",
      extensions: [
        history(),
        drawSelection(),
        rectangularSelection(),
        highlightActiveLine(),
        EditorView.lineWrapping,
        analysisField,
        clickHandler,
        // Bez toho webview obsah editoru na překlepy vůbec nekontroluje;
        // jestli se kontrola opravdu provádí, řídí nastavení v backendu.
        EditorView.contentAttributes.of({
          spellcheck: "true",
          autocorrect: "off",
          autocapitalize: "off",
        }),
        keymap.of([
          { key: "Mod-s", run: () => (handlers.onSave(), true), preventDefault: true },
          continueList,
          indentWithTab,
          ...historyKeymap,
          ...defaultKeymap,
        ]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) handlers.onChange(update.state.doc.toString());
        }),
        theme,
      ],
    }),
  });

  return view;
}

/** Nahradí obsah editoru (přepnutí poznámky) bez zápisu do historie undo. */
export function setContent(view, content) {
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: content },
    selection: { anchor: 0 },
    annotations: [],
  });
}

/**
 * Skok na řádek (1-based). `match` je nepovinný UTF-16 rozsah `{ from, to }`,
 * který se označí — po kliknutí do výsledků hledání je pak nález vidět rovnou
 * pod kurzorem.
 */
export function goToLine(view, line, match = null) {
  const target = Math.max(1, Math.min(line, view.state.doc.lines));
  const pos = view.state.doc.line(target).from;

  // Poznámka mohla mezitím na disku vyrůst nebo se zkrátit, takže se rozsah
  // vždycky ořezává na skutečnou délku dokumentu.
  const len = view.state.doc.length;
  const from = match ? Math.max(0, Math.min(match.from, len)) : pos;
  const to = match ? Math.max(from, Math.min(match.to, len)) : pos;

  view.dispatch({
    selection: { anchor: from, head: to },
    effects: EditorView.scrollIntoView(from, { y: "center" }),
  });
  view.focus();
}
