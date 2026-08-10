# Textura

Desktopový poznámkovač nad obyčejnými UTF-8 `.txt` soubory.

Poznámka musí zůstat dobře čitelná jako obyčejný text i v případě, že uživatel
tuto aplikaci už nikdy nespustí. Žádný Markdown, žádný povinný markup, žádný
skrytý stav v databázi — všechny vazby jsou rekonstruovatelné ze samotných
souborů.

Formát je popsaný v [docs/format.md](docs/format.md).

## Struktura

| Část | Co dělá |
| --- | --- |
| `crates/textura-core` | parser formátu, index workspace, resolver odkazů, backlinky, fulltext |
| `crates/textura-app` | Tauri backend — commandy mezi UI a jádrem |
| `ui/` | frontend: CodeMirror 6 + Vite |

Veškerá znalost formátu je v `textura-core`. Frontend si poznámku sám
neparsuje — zvýraznění dostává hotové z backendu, takže pravidla formátu
existují na jednom místě.

## Instalace závislostí

Rust (rustup) a Node 18+.

Na Debianu/Ubuntu jsou potřeba systémové knihovny pro Tauri v2:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev
```

Kontrola překlepů používá slovníky nainstalované v systému, například:

```sh
sudo apt install hunspell-cs hunspell-en-us
```

```sh
npm --prefix ui install
```

## Spuštění

```sh
cargo tauri dev            # potřebuje `cargo install tauri-cli --version '^2'`
```

nebo bez tauri-cli:

```sh
npm --prefix ui run dev &                 # frontend na :5173
cargo run -p textura-app                  # aplikace
```

Složku s poznámkami lze předat rovnou argumentem; jinak se otevře ta
naposledy použitá:

```sh
cargo run -p textura-app ~/Poznamky
```

Produkční build:

```sh
npm --prefix ui run build
cargo tauri build
```

## Testy

```sh
cargo test
```

Jádro je záměrně bez závislostí a `cargo test` staví jen `textura-core`
(viz `default-members` v [Cargo.toml](Cargo.toml)), takže parser se dá testovat
i tam, kde nejsou GTK/WebKit knihovny.

## Co aplikace umí

* otevřít složku s poznámkami, strom složek, přehled tagů
* zakládat poznámky i složky (včetně prázdných a víceúrovňových)
* filtrovat podle tagu a fulltextem, s lištou pro zrušení filtru
* nastavení: motiv (podle systému / světlý / tmavý) a kontrola překlepů
  nad systémovými slovníky
* editor se zvýrazněním `[[odkazů]]`, `#tagů`, `== sekcí ==`, úkolů, URL
  a properties
* <kbd>Ctrl</kbd>+<kbd>klik</kbd> na odkaz otevře poznámku; u nejednoznačného
  odkazu nabídne výběr, u neexistujícího nabídne vytvoření
* <kbd>Ctrl</kbd>+<kbd>klik</kbd> na URL otevře výchozí prohlížeč
* klik na `[ ]` překlikne úkol
* automatické pokračování seznamů a úkolů na Enter
* panel zpětných odkazů
* fulltext přes celý workspace
* autosave se zachováním původních konců řádků a BOM

## Co parser umí

* properties `@title:`, `@aliases:`, libovolné uživatelské klíče
* wiki odkazy `[[Docker]]`, cesty `[[Projects/Server]]`, aliasy `[[Docker|kontejnery]]`
* kotvy `[[Docker#Instalace]]` a stabilní ID `[[Docker#^docker-install]]`
* embedy `![[Docker]]` a přílohy `[[attachments/manual.pdf]]`
* sekce `== Nadpis ==`, volitelně `== Nadpis == ^id`
* tagy `#docker`, hierarchické `#server/linux` (a `C#` ani `#123` tagem nejsou)
* úkoly `[ ]`, `[x]`, `[!]` včetně atributů `@due:2026-08-15`
* automatickou detekci URL
* komentáře `%% ... %%`
* BOM, LF i CRLF, diakritiku a emoji bez posunu offsetů

```rust
use textura_core::parse;

let note = parse("@title: Docker\n\nDocker\n\n[[MariaDB]]\n\n#docker\n");
assert_eq!(note.title(), Some("Docker"));
assert_eq!(note.links[0].target, "MariaDB");
assert_eq!(note.tags[0].name, "docker");
```

## Stav

| Část | Stav |
| --- | --- |
| parser formátu | hotovo, 59 testů |
| index, resolver, složky, backlinky, fulltext | hotovo, 38 testů |
| Tauri backend | hotovo, 12 testů |
| UI | hotové pro MVP, testované ručně |
| přejmenování a přesouvání poznámek s aktualizací odkazů | plánováno |
| sledování změn na disku (watch) | plánováno |
| embedy `![[…]]` v preview | plánováno |
