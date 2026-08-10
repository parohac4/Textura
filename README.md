# Textura

Poznámkovač nad obyčejnými UTF-8 `.txt` soubory.

Poznámka musí zůstat dobře čitelná jako obyčejný text i v případě, že uživatel
tuto aplikaci už nikdy nespustí. Žádný Markdown, žádný povinný markup, žádný
skrytý stav v databázi — všechny vazby jsou rekonstruovatelné ze samotných
souborů.

Formát je popsaný v [docs/format.md](docs/format.md).

## Stav

Rozpracované. Hotový je parser formátu; UI (Tauri) zatím ne.

| Část | Stav |
| --- | --- |
| `crates/textura-core` — parser formátu | hotovo |
| resolver odkazů (konflikty názvů, aliasy) | plánováno |
| index a backlinky | plánováno |
| Tauri UI | plánováno |

## Vývoj

```sh
cargo test
```

Parser je záměrně bez závislostí, takže se přeloží i offline.

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

Příklad:

```rust
use textura_core::parse;

let note = parse("@title: Docker\n\nDocker\n\n[[MariaDB]]\n\n#docker\n");
assert_eq!(note.title(), Some("Docker"));
assert_eq!(note.links[0].target, "MariaDB");
assert_eq!(note.tags[0].name, "docker");
```
