# Instalace Textury na macOS

Návod pro cestu, kdy balíček staví GitHub a na Macu se jen instaluje. Nic se
na Macu nekompiluje, nemusí na něm být ani Rust, ani Node, ani Xcode.

Potřebuješ jen účet na GitHubu s přístupem k repozitáři a Mac s macOS 11
(Big Sur) nebo novějším.

Celé to zabere zhruba deset minut, z toho většinu odpracuje GitHub sám.

## Krok 1: Zjisti, jaký máš Mac

Levý horní roh → **⌘ menu** → *O tomto Macu*.

| Co je v řádku „Čip“ / „Procesor“ | Tvoje architektura | Který balíček stáhneš |
| --- | --- | --- |
| Apple M1, M2, M3, M4… | Apple Silicon | `Textura-macos-aarch64` |
| Intel Core i5, i7, i9… | Intel | `Textura-macos-x64` |

Zapamatuj si to, ve kroku 4 si podle toho vybereš.

## Krok 2: Spusť sestavení balíčku na GitHubu

Tohle jde udělat z jakéhokoli počítače, klidně z Linuxu.

1. Otevři <https://github.com/parohac4/Textura/actions>
2. V levém sloupci klikni na workflow **macOS balíček**.
3. Vpravo se objeví tlačítko **Run workflow**. Klikni na něj.
4. V rozbaleném okénku vyber v poli *Branch* větev **`macos`**.

   Tohle je důležité: dokud větev není sloučená do `main`, jinde workflow
   neexistuje.
5. Potvrď zeleným **Run workflow**.

Seznam běhů se do pár vteřin obnoví a nahoře přibude nový se žlutým kolečkem.
Když se neobjeví, načti stránku znovu.

## Krok 3: Počkej, až doběhne

Klikni na ten nový běh. Uvidíš dvě úlohy, jednu pro Apple Silicon a jednu pro
Intel. Trvá to zhruba pět až deset minut.

* **Zelená fajfka u obou** — hotovo, pokračuj krokem 4.
* **Červený křížek** — klikni na spadlou úlohu a rozklikni krok, u kterého je
  křížek; v jeho výpisu je napsáno, co se nepovedlo.

## Krok 4: Stáhni balíček

Zůstaň na stránce toho běhu a sjeď úplně dolů, do sekce **Artifacts**.
Jsou tam dvě položky:

```text
Textura-macos-aarch64      ← Apple Silicon (M1 a novější)
Textura-macos-x64          ← Intel
```

Klikni na tu svou podle tabulky z kroku 1 a stáhne se ZIP.

Stahuj rovnou na Macu, ať soubor nemusíš přenášet. Artefakty jsou dostupné
90 dní, pak je GitHub sám smaže.

## Krok 5: Nainstaluj aplikaci

Na Macu, ve složce Stažené soubory:

1. **Rozbal ZIP** — dvojklik. Vznikne z něj soubor `Textura_0.6.0_aarch64.dmg`
   (číslo verze a architektura se můžou lišit).
2. **Otevři `.dmg`** — dvojklik. Otevře se okno s ikonou Textury a se složkou
   Aplikace vedle ní.
3. **Přetáhni ikonu Textury na složku Aplikace.** To je celá instalace.
4. Okno zavři a `.dmg` v Finderu vysuň (šipka vedle jeho názvu v levém
   sloupci). Stažený ZIP i `.dmg` můžeš pak smazat.

## Krok 6: Povol první spuštění

Kdybys teď Texturu spustil, macOS řekne, že je poškozená nebo od neověřeného
vývojáře. Není to chyba stahování: balíček není podepsaný certifikátem od
Applu (stojí 99 dolarů ročně) a všechno takové stažené z internetu dostane
karanténní příznak.

Sundáš ho jedním příkazem. Otevři **Terminál** (⌘+mezerník, napiš `Terminál`,
Enter) a vlož:

```sh
xattr -dr com.apple.quarantine /Applications/Textura.app
```

Enter. Když příkaz nic nevypíše, proběhl v pořádku — tak se to má chovat.

Teď Texturu spusť: Launchpad, nebo ⌘+mezerník a napsat `Textura`.

Tohle se dělá **jednou**. Po restartu ani po dalším spuštění se to neopakuje.
Znovu to bude potřeba až u nově stažené verze.

## Krok 7: Otevři složku s poznámkami

Při prvním spuštění je okno prázdné. Tlačítkem pro otevření složky vyber
adresář se svými `.txt` poznámkami — nebo si založ nový prázdný, Textura si ho
naplní sama.

Vybranou složku si zapamatuje a příště ji otevře rovnou.

Poznámky zůstávají obyčejné `.txt` soubory. Nic se nekopíruje do knihovny
aplikace, takže složka může klidně ležet na iCloudu, Dropboxu nebo v Syncthingu
a poznámky sdílet s Linuxem.

## Aktualizace na novější verzi

Stejný postup: krok 2 až 6. Novou Texturu přetáhneš přes starou, Finder se
zeptá, jestli ji má nahradit — ano. A znovu ten `xattr` příkaz, protože stažený
balíček má karanténní příznak zase.

Nastavení ani poznámky se tím nijak nedotkneš.

Až bude větev `macos` sloučená do `main`, budou `.dmg` viset rovnou u vydání
na <https://github.com/parohac4/Textura/releases> a kroky 2 a 3 odpadnou — jen
stáhnout u příslušné verze.

## Odinstalace

Přetáhni `/Applications/Textura.app` do koše. To je vše, aplikace nikam jinam
nic neinstaluje.

Poznámky zůstanou tam, kde jsou. Smazat je musíš zvlášť, pokud chceš.

## Když něco nesedí

**„Run workflow“ na GitHubu nikde nevidím.** Musíš být přihlášený a mít
k repozitáři právo zápisu. A workflow musíš mít vybraný v levém sloupci —
na přehledu všech běhů to tlačítko není.

**V nabídce větví není `macos`.** Větev není na GitHubu. Zkontroluj
`git push -u origin macos`.

**Sekce Artifacts je prázdná.** Buď běh ještě neskončil, nebo spadl — artefakty
se objeví až na konci. A po 90 dnech je GitHub maže; pak stačí workflow spustit
znovu.

**„Textura.app je poškozená a měla by být přesunuta do koše“.** To je ten
karanténní příznak z kroku 6. Zkontroluj, že je aplikace opravdu ve složce
`/Applications` — příkaz míří tam. Když ji máš jinde, uprav v příkazu cestu.

**Aplikace nejde spustit ani po `xattr`.** Otevři Nastavení systému →
Soukromí a zabezpečení, sjeď dolů; bývá tam řádek o zablokované aplikaci
s tlačítkem *Přesto otevřít*.

**Kontrola překlepů nepodtrhává česky.** Jazyk si na macOS řídí systém, ne
Textura: Nastavení systému → Klávesnice → Zadávání textu → Pravopis, a přidej
češtinu. V nastavení aplikace jde kontrola jen zapnout a vypnout, seznam
slovníků tam proto na macOS není.

**Zkratky s Ctrl nefungují.** Na macOS je Textura poslouchá na ⌘, ne na Ctrl:
⌘+N nová poznámka, ⌘+F hledání, ⌘+M mind mapa, ⌘+klik otevře odkaz. Je to
schválně — Ctrl+klik znamená na macOS pravé tlačítko myši.
