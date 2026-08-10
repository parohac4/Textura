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
cd crates/textura-app && cargo tauri dev
```

Potřebuje `cargo install tauri-cli --version '^2' --locked`. Spouští se
z adresáře s `tauri.conf.json`, stejně jako build.

Na vývoj tohle stačí — balíček stavět nemusíš. Frontend se při změně překreslí
sám.

Nebo bez tauri-cli:

```sh
npm --prefix ui run dev &                 # frontend na :5173
cargo run -p textura-app                  # aplikace
```

Složku s poznámkami lze předat rovnou argumentem; jinak se otevře ta
naposledy použitá:

```sh
cargo run -p textura-app ~/Poznamky
```

## Vydání nové verze

Běžná práce se commituje a pushuje jako obvykle. Push sám o sobě **nijak
neaktualizuje nainstalovanou aplikaci** — `.deb` je zmrazený snímek binárky.
Novou verzi vydáš takhle:

```sh
git push                                                        # běžné commity
./release.sh 0.2.0                                              # vydání
sudo apt install ./target/release/bundle/deb/Textura_0.2.0_amd64.deb
```

[release.sh](release.sh) projde celý postup: kontroly → zvýšení verze → testy →
balíčky → commit → tag → push → GitHub release. Popis tagu poskládá z commitů
od minulého tagu. Před pushem se zeptá.

```sh
./release.sh 0.2.0 --dry-run    # jen vypíše plán, nic nezmění
./release.sh 0.2.0 --yes        # nezeptá se před pushem
```

Skript odmítne pokračovat, pokud nejsi na `main`, pracovní strom není čistý,
tag už existuje, `gh` není přihlášené nebo je `origin/main` napřed. Když
selžou testy nebo build, vrátí změnu verze zpátky.

Poslední krok — instalaci — skript nedělá schválně, protože vyžaduje `sudo`.
Vypíše ti ho na konci k zkopírování.

Co skript nehlídá: jestli číslo verze odpovídá povaze změn. Volba mezi
patch, minor a major zůstává na tobě.

### Kde se verze bere

Jen na jednom místě — `[workspace.package] version` v [Cargo.toml](Cargo.toml).
`tauri.conf.json` ji záměrně neuvádí a Tauri si ji z `Cargo.toml` vezme samo.
Dřív byla na dvou místech a nic nehlídalo, že se shodují, takže zvýšení jen
jednoho z nich tiše vyrobilo balíček s jinou verzí než crate.

### Zjištění verze a odinstalace

Ověřit, co je nainstalované:

```sh
dpkg -s textura | grep Version
```

Odinstalovat:

```sh
sudo apt remove textura
```

## Balíčky ručně

```sh
cd crates/textura-app && cargo tauri build
```

Vyrobí `.deb`, `.rpm` a AppImage v `target/release/bundle/`.

Aktualizace nainstalované aplikace:

```sh
sudo apt install ./target/release/bundle/deb/Textura_0.1.0_amd64.deb
```

Pozor na cesty v `tauri.conf.json`: `frontendDist` je relativní k samotnému
konfiguračnímu souboru, ale `beforeDevCommand` a `beforeBuildCommand` se
spouštějí z **nadřazené** složky (Tauri počítá s rozvržením `projekt/src-tauri/`).
Proto `../../ui/dist`, ale `--prefix ../ui`.

Ruční produkční build bez tauri-cli potřebuje feature `custom-protocol` —
bez ní by i release build hledal frontend na vývojovém serveru:

```sh
npm --prefix ui run build
cargo build --release -p textura-app --features custom-protocol
```

Výsledkem je samostatná binárka `target/release/textura-app` s frontendem
zapečeným uvnitř.

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
