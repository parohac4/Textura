// Vykreslení a úpravy mind mapy.
//
// Mapa je v poznámce obyčejný odsazený seznam pod řádkem `@mapa`; tady se z něj
// kreslí bubliny a naopak se z nich skládá seznam uzlů, který backend zapíše
// zpět do textu. Formát tenhle soubor nezná — dostává hotové uzly z `analyze`
// a posílá je zpátky do `render_mindmap`.
//
// Bublina, kterou uživatel odtáhl myší, má uloženou pozici; ostatní se srovnají
// automaticky podle zanoření.

const NODE_H = 32;
const GAP_Y = 12;
const GAP_X = 52;
const ROOT_GAP = 22;
const PAD_X = 14;
/** Přibližná šířka znaku písma uzlu — na odhad šířky bubliny to stačí. */
const CHAR_W = 7.6;
const MAX_CHARS = 34;
/** Okraj kolem mapy, aby se odtažená bublina nelepila na kraj plátna. */
const PAD = 32;

function shorten(label) {
  const text = label.trim() || "…";
  return text.length > MAX_CHARS ? `${text.slice(0, MAX_CHARS - 1)}…` : text;
}

/**
 * Rozmístí bubliny.
 *
 * Automatické rozvržení je strom zleva doprava: sloupec odpovídá zanoření, list
 * si bere další volný pás a rodič se svisle srovná na střed svých potomků.
 * Uzel s uloženou pozicí ji dostane přednostně — uživatel si bubliny může
 * rozházet, jak chce, a struktura mapy tím zůstane nedotčená.
 */
export function layout(map) {
  const nodes = map.nodes.map((node) => {
    const label = shorten(node.label);
    return {
      ...node,
      label,
      w: Math.max(76, label.length * CHAR_W + PAD_X * 2),
      h: NODE_H,
      x: 0,
      y: 0,
      // Uložená pozice se dosadí až po výpočtu rozvržení, aby automatické
      // srovnání zbytku mapy nezáviselo na tom, co je odtažené.
      savedX: node.x,
      savedY: node.y,
      placed: node.x != null && node.y != null,
    };
  });

  const children = nodes.map(() => []);
  const roots = [];
  nodes.forEach((node, i) => {
    if (node.parent == null) roots.push(i);
    else children[node.parent].push(i);
  });

  const columnWidth = [];
  for (const node of nodes) {
    columnWidth[node.depth] = Math.max(columnWidth[node.depth] ?? 0, node.w);
  }
  const columnX = [];
  let x = 0;
  for (let depth = 0; depth < columnWidth.length; depth += 1) {
    columnX[depth] = x;
    x += (columnWidth[depth] ?? 0) + GAP_X;
  }

  let cursor = 0;
  const place = (i) => {
    const kids = children[i];
    if (kids.length) {
      for (const kid of kids) place(kid);
      const first = nodes[kids[0]];
      const last = nodes[kids[kids.length - 1]];
      nodes[i].y = (first.y + last.y + last.h - nodes[i].h) / 2;
    } else {
      nodes[i].y = cursor;
      cursor += NODE_H + GAP_Y;
    }
    nodes[i].x = columnX[nodes[i].depth];
  };
  roots.forEach((root, index) => {
    if (index) cursor += ROOT_GAP;
    place(root);
  });

  // Odtažená bublina si bere svoji pozici a **posune s sebou i potomky**, kteří
  // vlastní pozici nemají — jinak by po odtažení rodiče zůstal jeho podstrom
  // rozházený někde v původním sloupci. Uzly jsou v pořadí ze souboru, takže
  // rodič je vždy zpracovaný dřív než jeho potomek.
  const shift = nodes.map(() => ({ x: 0, y: 0 }));
  nodes.forEach((node, i) => {
    const inherited = node.parent == null ? { x: 0, y: 0 } : shift[node.parent];
    if (node.placed) {
      shift[i] = { x: node.savedX - node.x, y: node.savedY - node.y };
      node.x = node.savedX;
      node.y = node.savedY;
      return;
    }
    shift[i] = inherited;
    node.x += inherited.x;
    node.y += inherited.y;
  });

  const minX = Math.min(0, ...nodes.map((n) => n.x));
  const minY = Math.min(0, ...nodes.map((n) => n.y));
  const width = nodes.reduce((max, n) => Math.max(max, n.x + n.w), 0) - minX;
  const height = nodes.reduce((max, n) => Math.max(max, n.y + n.h), 0) - minY;
  return { nodes, children, roots, minX, minY, width, height };
}

const svgEl = (name, attrs = {}) => {
  const el = document.createElementNS("http://www.w3.org/2000/svg", name);
  for (const [key, value] of Object.entries(attrs)) el.setAttribute(key, value);
  return el;
};

/** Střed bubliny. */
const center = (node) => ({ x: node.x + node.w / 2, y: node.y + node.h / 2 });

/**
 * Bod na okraji bubliny ve směru k `to` — čára tak končí u rámečku, ne uvnitř.
 *
 * Bubliny se dají rozházet libovolně, takže se spoj nedá kreslit vždy „zprava
 * doleva“ jako u srovnaného stromu.
 */
function border(node, to) {
  const from = center(node);
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  if (dx === 0 && dy === 0) return from;

  const halfW = node.w / 2;
  const halfH = node.h / 2;
  // Poměr, ve kterém úsečka opustí obdélník.
  const scale = Math.min(
    dx === 0 ? Infinity : halfW / Math.abs(dx),
    dy === 0 ? Infinity : halfH / Math.abs(dy),
  );
  return { x: from.x + dx * scale, y: from.y + dy * scale };
}

function edgePath(a, b) {
  const from = border(a, center(b));
  const to = border(b, center(a));
  return { from, to, d: `M${from.x},${from.y} L${to.x},${to.y}` };
}

/**
 * Nakreslí mapu do kontejneru.
 *
 * @param map       mapa z analýzy (`analysis.mindmaps[i]`)
 * @param links     `analysis.links` — uzel na svůj odkaz ukazuje indexem
 * @param options   { selected, selectedEdge, connectFrom, zoom, handlers }
 */
export function draw(container, map, links, options) {
  const { selected, selectedEdge, connectFrom, zoom = 1, handlers } = options;
  container.replaceChildren();

  if (!map.nodes.length) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = "Mapa je prázdná — přidej první buňku.";
    container.append(empty);
    return null;
  }

  const tree = layout(map);
  const viewX = tree.minX - PAD;
  const viewY = tree.minY - PAD;
  const viewW = tree.width + PAD * 2;
  const viewH = tree.height + PAD * 2;

  const svg = svgEl("svg", {
    class: "map-svg",
    width: viewW * zoom,
    height: viewH * zoom,
    viewBox: `${viewX} ${viewY} ${viewW} ${viewH}`,
  });
  svg.dataset.zoom = String(zoom);

  // Spoje ze zanoření: jeden na každý uzel s rodičem.
  const edges = svgEl("g", { class: "map-edges" });
  for (const node of tree.nodes) {
    if (node.parent == null) continue;
    const { d } = edgePath(tree.nodes[node.parent], node);
    edges.append(svgEl("path", { class: "map-edge", d }));
  }

  // Ruční spoje: čárkovaně, aby bylo poznat, co je struktura a co spojka.
  map.edges.forEach((edge, index) => {
    if (edge.fromNode == null || edge.toNode == null) return;
    const { d } = edgePath(tree.nodes[edge.fromNode], tree.nodes[edge.toNode]);
    const group = svgEl("g", {
      class: `map-link-edge${index === selectedEdge ? " selected" : ""}`,
      "data-edge": String(index),
    });
    // Neviditelná tlustá čára pod tou vykreslenou — na tenkou linku se klikat
    // nedá.
    group.append(svgEl("path", { class: "map-edge-hit", d }));
    group.append(svgEl("path", { class: "map-edge-manual", d }));
    group.addEventListener("mousedown", (event) => {
      event.stopPropagation();
      handlers.onSelectEdge(index);
    });
    edges.append(group);
  });
  svg.append(edges);

  tree.nodes.forEach((node, index) => {
    const link = node.link == null ? null : links[node.link];
    const group = svgEl("g", {
      class: [
        "map-node",
        index === selected ? "selected" : "",
        index === connectFrom ? "connecting" : "",
        node.placed ? "placed" : "",
        link ? `has-link link-${link.state}` : "",
      ]
        .filter(Boolean)
        .join(" "),
      transform: `translate(${node.x},${node.y})`,
      "data-index": String(index),
    });
    // Barva se ukládá názvem, odstín dosazuje motiv — stejně jako u poznámek.
    if (node.color) group.dataset.color = node.color;

    group.append(svgEl("rect", { class: "map-box", width: node.w, height: node.h, rx: 8 }));

    const text = svgEl("text", { class: "map-label", x: PAD_X, y: node.h / 2 + 5 });
    text.textContent = node.label;
    group.append(text);

    if (link) {
      const open = svgEl("text", { class: "map-open", x: node.w - 12, y: node.h / 2 + 5 });
      open.textContent = "↗";
      const tip = svgEl("title", {});
      tip.textContent =
        link.state === "unresolved"
          ? `${link.display} — poznámka zatím neexistuje`
          : `Otevřít ${link.display}`;
      open.append(tip);
      open.addEventListener("mousedown", (event) => {
        event.stopPropagation();
        handlers.onOpenLink(index, link);
      });
      group.append(open);
    }

    const title = svgEl("title", {});
    title.textContent = node.id ? `${node.text}  ^${node.id}` : node.text;
    group.append(title);

    group.addEventListener("mousedown", (event) => {
      event.preventDefault();
      handlers.onNodeDown(index, event);
    });
    // Buňka s odkazem se dvojklikem neupravuje — první klik už otevřel
    // poznámku. Úprava je na F2 a v menu pod pravým klikem.
    if (!link) group.addEventListener("dblclick", () => handlers.onEdit(index));
    group.addEventListener("contextmenu", (event) => handlers.onMenu(index, event));

    svg.append(group);
  });

  svg.addEventListener("mousedown", (event) => {
    // Klik do prázdna zruší výběr spoje i rozdělané spojování.
    if (event.target === svg) handlers.onBackgroundDown(event);
  });

  container.append(svg);
  return tree;
}

/**
 * Přebarví vybraný uzel bez překreslení mapy.
 *
 * Kdyby se mapa kreslila znovu při každém výběru, klik na ↗ ani dvojklik by se
 * nedokončily — prvek, na kterém začaly, by mezitím zmizel.
 */
export function markSelected(container, index) {
  for (const node of container.querySelectorAll(".map-node")) {
    node.classList.toggle("selected", Number(node.dataset.index) === index);
  }
}

/**
 * Posune bublinu během tažení: jen vykreslení, do textu se nic nezapisuje.
 * Vrací novou pozici v souřadnicích mapy.
 */
export function moveNodeVisual(container, index, dx, dy, start) {
  const group = container.querySelector(`.map-node[data-index="${index}"]`);
  if (!group) return null;
  const x = Math.round(start.x + dx);
  const y = Math.round(start.y + dy);
  group.setAttribute("transform", `translate(${x},${y})`);
  group.classList.add("dragging");
  return { x, y };
}

/** Přepočítá pixely na obrazovce na souřadnice v mapě (kvůli zvětšení). */
export function scaleOf(container) {
  const svg = container.querySelector(".map-svg");
  return svg ? Number(svg.dataset.zoom) || 1 : 1;
}

// --- úpravy stromu ---------------------------------------------------------
//
// Uzly se upravují jako plochý seznam `{ depth, text, id }` — přesně to, co umí
// backend zapsat zpátky do poznámky.

export function toList(map) {
  return map.nodes.map((node) => ({
    depth: node.depth,
    text: node.text,
    color: node.color ?? null,
    id: node.id ?? null,
  }));
}

/** Ruční spoje jako dvojice ID, tedy tak, jak je bere backend. */
export function toEdges(map) {
  return map.edges.map((edge) => ({ from: edge.from, to: edge.to }));
}

/** Kolik položek zabírá uzel se svými potomky. */
export function subtreeLength(list, index) {
  let end = index + 1;
  while (end < list.length && list[end].depth > list[index].depth) end += 1;
  return end - index;
}

/** Vloží nový uzel za podstrom `index`, o `shift` úrovní hlouběji. */
export function insertNode(list, index, shift, text) {
  const next = [...list];
  if (!next.length) {
    next.push({ depth: 0, text, id: null });
    return { list: next, select: 0 };
  }
  const at = index + subtreeLength(list, index);
  const depth = Math.max(0, list[index].depth + shift);
  next.splice(at, 0, { depth, text, id: null });
  return { list: next, select: at };
}

/**
 * Smaže uzel i s potomky. Ruční spoje, které na smazané uzly mířily, jdou
 * s nimi — čára do prázdna by v mapě jen zůstala viset.
 */
export function removeNode(list, edges, index) {
  const count = subtreeLength(list, index);
  const goneIds = list
    .slice(index, index + count)
    .map((node) => node.id)
    .filter(Boolean)
    .map((id) => id.toLowerCase());

  const next = [...list];
  next.splice(index, count);
  return {
    list: next,
    edges: edges.filter(
      (edge) =>
        !goneIds.includes(edge.from.toLowerCase()) && !goneIds.includes(edge.to.toLowerCase()),
    ),
    select: Math.min(index, Math.max(0, next.length - 1)),
    removedIds: goneIds,
  };
}

export function setText(list, index, text) {
  const next = [...list];
  next[index] = { ...next[index], text };
  return { list: next, select: index };
}

/** Dá uzlu stabilní ID, aby šel spojit nebo si zapamatovat pozici. */
export function setId(list, index, id) {
  const next = [...list];
  next[index] = { ...next[index], id };
  return { list: next, select: index };
}

/** Nastaví barvu buňky; `null` ji zruší. */
export function setColor(list, index, color) {
  const next = [...list];
  next[index] = { ...next[index], color };
  return { list: next, select: index };
}

/** ID, která jsou v mapě zabraná. */
export function takenIds(list) {
  return list.map((node) => node.id).filter(Boolean);
}

/** Přidá ruční spoj, pokud tam ještě není. */
export function addEdge(edges, from, to) {
  const same = (a, b) => a.toLowerCase() === b.toLowerCase();
  if (same(from, to)) return null;
  const exists = edges.some(
    (edge) =>
      (same(edge.from, from) && same(edge.to, to)) || (same(edge.from, to) && same(edge.to, from)),
  );
  if (exists) return null;
  return [...edges, { from, to }];
}

export function removeEdge(edges, index) {
  const next = [...edges];
  next.splice(index, 1);
  return next;
}

/** Předchozí sourozenec uzlu, nebo `-1`. */
function previousSibling(list, index) {
  const depth = list[index].depth;
  for (let i = index - 1; i >= 0; i -= 1) {
    if (list[i].depth < depth) return -1;
    if (list[i].depth === depth) return i;
  }
  return -1;
}

/** Zanoří uzel i s potomky pod předchozího sourozence. */
export function indentNode(list, index) {
  if (previousSibling(list, index) < 0) return null;
  return shiftSubtree(list, index, 1);
}

/** Vytáhne uzel o úroveň výš. Kořen už výš nejde. */
export function outdentNode(list, index) {
  if (list[index].depth === 0) return null;
  return shiftSubtree(list, index, -1);
}

function shiftSubtree(list, index, by) {
  const length = subtreeLength(list, index);
  const next = list.map((node, i) =>
    i >= index && i < index + length ? { ...node, depth: node.depth + by } : node,
  );
  return { list: next, select: index };
}

/** Posune uzel mezi sourozenci nahoru (`-1`) nebo dolů (`1`). */
export function moveNode(list, index, direction) {
  const depth = list[index].depth;
  const length = subtreeLength(list, index);

  if (direction < 0) {
    const previous = previousSibling(list, index);
    if (previous < 0) return null;
    const next = [...list];
    const moved = next.splice(index, length);
    next.splice(previous, 0, ...moved);
    return { list: next, select: previous };
  }

  const after = index + length;
  if (after >= list.length || list[after].depth < depth) return null;
  const nextLength = subtreeLength(list, after);
  const next = [...list];
  const moved = next.splice(index, length);
  next.splice(index + nextLength, 0, ...moved);
  return { list: next, select: index + nextLength };
}

/** Kam se má posunout výběr při šipkách. Vrací index, nebo `null`. */
export function navigate(map, index, key) {
  const node = map.nodes[index];
  if (!node) return null;

  if (key === "ArrowRight") {
    const child = map.nodes.findIndex((n) => n.parent === index);
    return child < 0 ? null : child;
  }
  if (key === "ArrowLeft") {
    return node.parent ?? null;
  }

  const step = key === "ArrowDown" ? 1 : -1;
  // Nejbližší uzel stejné úrovně ve směru pohybu — ve srovnané mapě jsou
  // takové uzly svisle nad sebou, takže to odpovídá tomu, co uživatel vidí.
  for (let i = index + step; i >= 0 && i < map.nodes.length; i += step) {
    if (map.nodes[i].depth === node.depth) return i;
  }
  return null;
}
