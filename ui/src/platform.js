// Rozdíly mezi platformami, které se týkají jen UI.
//
// Backend zná systém přesně (command `platform`), ale tyhle drobnosti musí
// být k dispozici hned a synchronně — než doběhne první `invoke`, editor už
// stojí a popisky jsou vykreslené. Proto se tu ptáme prohlížeče.

/** Běžíme na macOS? */
export const IS_MAC = /Mac|iPhone|iPad/.test(
  navigator.userAgentData?.platform || navigator.platform || navigator.userAgent,
);

/**
 * Modifikátor, kterým se aktivují odkazy a zkratky.
 *
 * Na macOS je to Cmd, ne Ctrl: Ctrl+klik tam systém bere jako pravé tlačítko,
 * takže by se s ním místo otevření odkazu vyrolovalo kontextové menu.
 */
export const MOD_LABEL = IS_MAC ? "⌘" : "Ctrl";

/**
 * Je u téhle události (klik nebo klávesa) stisknutý modifikátor?
 *
 * Na macOS schválně jen Cmd. Ctrl tam znamená pravé tlačítko myši a patří
 * textovým zkratkám systému (Ctrl+N o řádek dolů, Ctrl+F o znak vpravo);
 * brát mu je by bylo protivné a Ctrl+klik by místo odkazu otevřel menu.
 */
export const isMod = (event) => (IS_MAC ? event.metaKey : event.ctrlKey || event.metaKey);

/**
 * Přepíše „Ctrl“ na „⌘“ v popiscích napsaných natvrdo v `index.html`.
 *
 * Držet dvě jazykové verze staticky napsaného HTML by znamenalo popisky
 * duplikovat; takhle zůstávají na jednom místě a na macOS se jen přeznačí.
 */
export function localizeShortcutLabels(root = document) {
  if (!IS_MAC) return;

  for (const element of root.querySelectorAll("[title], [placeholder]")) {
    for (const attribute of ["title", "placeholder"]) {
      const value = element.getAttribute(attribute);
      if (value?.includes("Ctrl")) element.setAttribute(attribute, value.replaceAll("Ctrl", "⌘"));
    }
  }

  for (const kbd of root.querySelectorAll("kbd")) {
    if (kbd.textContent === "Ctrl") kbd.textContent = "⌘";
  }
}
