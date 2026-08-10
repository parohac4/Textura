// Nastavení aplikace. Drží se v localStorage — je to čistě věc UI, do
// poznámek ani do workspace nezasahuje.

const KEY = "textura:settings";

const DEFAULTS = {
  /** "system" | "light" | "dark" */
  theme: "system",
  /** Kontrola překlepů zapnutá. */
  spellCheck: true,
  /**
   * Jazyky kontroly. `null` znamená „ještě nevybráno“ — při prvním spuštění
   * se doplní podle slovníků dostupných v systému.
   */
  spellLanguages: null,
};

export function loadSettings() {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(KEY) ?? "{}") };
  } catch {
    return { ...DEFAULTS };
  }
}

export function saveSettings(settings) {
  localStorage.setItem(KEY, JSON.stringify(settings));
}

/**
 * Aplikuje motiv na dokument.
 *
 * "system" nechá rozhodnout `prefers-color-scheme`, proto atribut odstraňuje
 * — jakákoli hodnota by systémovou volbu přebila.
 */
export function applyTheme(theme) {
  if (theme === "light" || theme === "dark") {
    document.documentElement.dataset.theme = theme;
  } else {
    delete document.documentElement.dataset.theme;
  }
}
