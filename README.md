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
* tlačítko **+ Datum** (<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>N</kbd>) založí bez
  ptaní poznámku pojmenovanou aktuálním datem a časem, a to ve složce právě
  otevřené poznámky
* pravý klik na poznámku nebo složku otevře menu s přejmenováním, přesunem,
  barvou a mazáním
* přejmenovat a přesouvat poznámky i složky, vnořovat složky do sebe
* obarvit poznámky i složky osmi barvami
* filtrovat podle tagu a fulltextem, s lištou pro zrušení filtru
* <kbd>Ctrl</kbd>+<kbd>F</kbd> skočí do hledání, <kbd>Esc</kbd> ho zruší
  a vrátí kurzor do poznámky
* nastavení: motiv (podle systému / světlý / tmavý) a kontrola překlepů
  nad systémovými slovníky
* editor se zvýrazněním `[[odkazů]]`, `#tagů`, `== sekcí ==`, úkolů, URL
  a properties
* <kbd>Ctrl</kbd>+<kbd>klik</kbd> na odkaz otevře poznámku; u nejednoznačného
  odkazu nabídne výběr, u neexistujícího nabídne vytvoření
* <kbd>Ctrl</kbd>+<kbd>klik</kbd> na URL otevře výchozí prohlížeč
* klik na `[ ]` překlikne úkol
* automatické pokračování seznamů a úkolů na Enter
* mind mapy: blok `@mapa` s odsazeným seznamem se dá otevřít jako schéma
  (<kbd>Ctrl</kbd>+<kbd>M</kbd> nebo tlačítko **Mapa**), tahat v něm bubliny
  myší, obarvovat je, mazat a ručně spojovat čárami
* panel zpětných odkazů
* fulltext přes celý workspace — bez ohledu na diakritiku, víc slov jako AND,
  se zvýrazněným nálezem
* autosave se zachováním původních konců řádků a BOM

### Přesun a mazání

Přesun poznámky nebo složky **opraví odkazy s celou cestou** —
`[[Projekty/Docker#Instalace|instalaci]]` se stane
`[[Servery/Docker#Instalace|instalaci]]`, kotva, popisek i embed zůstanou.
Opraví se i odkazy mezi poznámkami uvnitř přesouvané složky.

Odkazy holým názvem `[[Docker]]` se nepřepisují — po přesunu dál fungují,
protože se dohledávají podle názvu. Po operaci stavový řádek napíše, kolik
odkazů se opravilo a kolik jich míří jen názvem. Při **přejmenování** se z nich
stanou nerozřešené odkazy, na což dialog dopředu upozorní.

Přejmenování je tatáž operace jako přesun, takže se odkazy opravují stejně.

### Barvy

Poznámka i složka můžou dostat jednu z osmi barev. Ukládá se **název** barvy,
ne odstín — ten dosazuje motiv, aby zůstala čitelná ve světlém i tmavém.

Poznámka si barvu nese sama jako property přímo v souboru:

```text
@color: blue
```

Je tedy vidět i ve `vim`u, dá se najít `grep`em a přežije přesun i
synchronizaci. Nastavení barvy proto poznámku přepíše; pokud žádné properties
neměla, dostane blok na začátku.

Složka vlastní soubor nemá, takže její barva jde do `.txtnote/colors.txt`:

```text
Projekty = blue
Servery/Produkce = green
```

Mimo poznámky jsou uložené jen dvě věci: barvy složek a pozice bublin
v mind mapách (`.txtnote/mindmap.txt`). Když se `.txtnote/` smaže, ztratí se
tyhle dvě ozdoby — nic jiného. Barva složky se s ní posouvá při přesunu
i přejmenování a při smazání se ze souboru uklidí.

Hodnotu mimo paletu aplikace ignoruje, takže si `@color` můžeš přepsat na
cokoli, aniž by se něco rozbilo.

### Mind mapy

Mapa se do poznámky zapisuje jako obyčejný odsazený seznam pod řádkem `@mapa`:

```text
@mapa Infrastruktura
- Produkční server ^server
  - [[Docker]] @color:blue ^docker
    - běží v něm pět služeb
  - [[MariaDB|databáze]] ^db
@spoje
- docker -- db
```

Do buňky patří vlastní popis, odkaz na poznámku, nebo obojí. Odkaz v buňce je
běžný odkaz — dohledává se stejně a počítá se do zpětných odkazů, takže mapa
není ostrůvek mimo zbytek workspace.

<kbd>Ctrl</kbd>+<kbd>M</kbd> (nebo <kbd>Ctrl</kbd>+<kbd>klik</kbd> na řádek
`@mapa`) přepne poznámku do schématu.

| Akce | Jak |
| --- | --- |
| přesunout bublinu | tahem myší |
| otevřít poznámku buňky | kliknutím na buňku (nebo na ↗) |
| nová podřízená / sousední buňka | <kbd>Tab</kbd> / <kbd>Enter</kbd> |
| upravit buňku | <kbd>F2</kbd>, dvojklik nebo menu na pravý klik |
| barva buňky | <kbd>B</kbd> nebo menu |
| spojit dvě buňky čárou | <kbd>S</kbd> a pak klik na druhou buňku |
| smazat buňku i s potomky | <kbd>Delete</kbd> |
| smazat ruční spoj | klik na čáru a <kbd>Delete</kbd> |
| přesun v osnově | <kbd>Alt</kbd>+šipky |
| výběr buňky | šipky |
| srovnat rozházené bubliny | tlačítko **Srovnat** |
| zpět do textu | <kbd>Esc</kbd> |

Ručně spojit jde **kterákoli buňka s kteroukoli jinou** a buněk se dá spojovat
víc mezi sebou — čáry ze zanoření tím zůstávají nedotčené, ruční spoje se kreslí
čárkovaně. Spoj se v souboru odkazuje na stabilní ID buňky, takže si aplikace ID
doplní sama, jakmile buňku spojíš nebo její bublinu odtáhneš.

Každá úprava jde zpátky do textu poznámky, takže se ukládá autosavem, vrací
<kbd>Ctrl</kbd>+<kbd>Z</kbd> a je vidět i ve `vim`u. Poznámka může mít map víc
a přepíná se mezi nimi rozbalovacím seznamem v liště mapy.

Jediné, co v textu není, jsou **souřadnice odtažených bublin** — to není obsah,
ale vzhled, takže bydlí v `.txtnote/mindmap.txt`. Po jeho smazání se mapa
srovná podle odsazení a nic dalšího se neztratí. Buňka bez uložené pozice se
posouvá s odtaženým rodičem, aby po přetažení nezůstal podstrom rozházený.

### Koš

Mazání není nevratné: poznámka nebo složka se přesune do
`.txtnote/trash/<časové razítko>/` uvnitř workspace. Soubory tam zůstávají
čitelné jako obyčejný text a vrátíš je prostým přesunutím zpět. Aplikace koš
neindexuje. Neprázdnou složku smaže jen po výslovném potvrzení, které říká,
kolika poznámek se to týká.

### Hledání

<kbd>Ctrl</kbd>+<kbd>F</kbd> skočí do hledacího pole, <kbd>Esc</kbd> hledání
zruší a vrátí kurzor do rozepsané poznámky.

Porovnává se **bez ohledu na velikost písmen a na diakritiku**, takže `zluty`
najde *Žluťoučký* a `manana` najde *mañana* — kdo píše česky, nemusí kvůli
hledání přepínat klávesnici. Zvýrazní se přitom původní zápis, diakritika
i velká písmena.

Výrazy oddělené mezerou platí jako **AND**: `docker mariadb` najde poznámky,
ve kterých je obojí, ať je pořadí jakékoli a ať jsou na stejném řádku nebo ne.
Vypíšou se pak řádky s kterýmkoli z výrazů, aby byl vidět kontext obou stran
dotazu.

Hledá se i v **názvech** (`@title`, jinak jméno souboru) a v **aliasech**;
takové nálezy jdou první a nesou odznak *název* / *alias*. Kdo hledá `docker`,
chce nejspíš poznámku *Docker*, ne dvacet zmínek o ní.

Nález je zvýrazněný ve výpisu výsledků a po kliknutí i označený v editoru, takže
je hned vidět, na co se kliklo.

## Co parser umí

* properties `@title:`, `@aliases:`, libovolné uživatelské klíče
* wiki odkazy `[[Docker]]`, cesty `[[Projects/Server]]`, aliasy `[[Docker|kontejnery]]`
* kotvy `[[Docker#Instalace]]` a stabilní ID `[[Docker#^docker-install]]`
* embedy `![[Docker]]` a přílohy `[[attachments/manual.pdf]]`
* sekce `== Nadpis ==`, volitelně `== Nadpis == ^id`
* mind mapy `@mapa` s odsazeným seznamem uzlů, stabilními ID `^id`, barvami
  `@color:` a ručními spoji v bloku `@spoje`
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
| mind mapy `@mapa` (ID, spoje, barvy, pozice) | hotovo, 27 testů v jádře |
| index, resolver, složky, backlinky, fulltext | hotovo, 81 testů |
| skládání textu pro hledání (diakritika, dotaz) | hotovo, 8 testů |
| Tauri backend | hotovo, 17 testů |
| UI včetně mind map | hotové pro MVP, testované ručně |
| přejmenování a přesouvání poznámek s aktualizací odkazů | plánováno |
| sledování změn na disku (watch) | plánováno |
| embedy `![[…]]` v preview | plánováno |
