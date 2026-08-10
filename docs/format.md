# Formát `.txt` poznámek

Aplikace pracuje výhradně s obyčejnými UTF-8 `.txt` soubory.

Cílem je zachovat maximální čitelnost poznámek i bez samotné aplikace. Uživatel musí být schopný otevřít libovolnou poznámku v běžném textovém editoru a její obsah musí dávat smysl.

Aplikace nesmí vyžadovat Markdown syntax.

## Základní pravidla

Každá poznámka je jeden `.txt` soubor.

Například:

```text
Docker.txt
```

Obsah:

```text
Docker

Docker používám pro provoz jednotlivých služeb.

Související poznámky:
[[MariaDB]]
[[Mautic]]
[[Linux]]

#docker
#server
```

Název souboru představuje výchozí název poznámky.

Přípona `.txt` se v uživatelském rozhraní standardně nemusí zobrazovat.

---

## Wiki odkazy

Interní odkazy používají syntaxi:

```text
[[Název poznámky]]
```

Například:

```text
[[Docker]]
```

odkazuje na:

```text
Docker.txt
```

Přípona `.txt` se v odkazu nepíše.

Podporuj také cestu ke konkrétní poznámce:

```text
[[Projects/Server]]
```

která odkazuje například na:

```text
Projects/Server.txt
```

### Alias odkazu

Podporuj alias:

```text
[[Docker|kontejnerová platforma]]
```

Uživatel v editoru uvidí text:

```text
kontejnerová platforma
```

ale odkaz stále směřuje na:

```text
Docker.txt
```

Parser musí oddělovat:

```text
target = Docker
label = kontejnerová platforma
```

---

## Odkazy na nadpisy nebo části poznámky

Aplikace může podporovat jednoduché interní sekce.

Protože nechceme Markdown, použij vlastní jednoduchou syntaxi nadpisů:

```text
== Instalace ==
```

Například:

```text
Docker

== Instalace ==

Postup instalace Dockeru...

== Networking ==

Informace o sítích...
```

Odkaz na konkrétní sekci:

```text
[[Docker#Instalace]]
```

Alias:

```text
[[Docker#Instalace|instalace Dockeru]]
```

Sekce musí být dohledatelná case-insensitive, ale aplikace by měla zachovávat původní zápis.

---

## Stabilní ID sekcí

Pokud vznikne problém s přejmenováním sekcí, architektura může později podporovat stabilní block/section ID.

Například:

```text
== Instalace == ^docker-install
```

Odkaz:

```text
[[Docker#^docker-install]]
```

Tato funkce není nutná pro první MVP.

---

## Tagy

Tag používá syntaxi:

```text
#docker
```

Například:

```text
#docker
#linux
#server
```

Podporuj také hierarchické tagy:

```text
#server/linux
#project/mautic
```

Platný tag začíná znakem `#`.

Parser nesmí interpretovat jako tag například:

```text
C#
#123
```

pokud to nesplňuje definovaná pravidla tagu.

Doporučené pravidlo:

tag musí začínat písmenem nebo Unicode znakem představujícím písmeno.

Příklad platných tagů:

```text
#linux
#server2026
#projekt/mautic
#fotografie
```

---

## Properties / metadata poznámky

Pokud bude potřeba ukládat metadata přímo do poznámky, nepoužívej YAML front matter.

Použij jednoduchý plain-text blok.

Například:

```text
@title: Docker
@created: 2026-08-10
@modified: 2026-08-10
@status: active
@aliases: Containers, Docker Engine
```

Metadata musí být volitelná.

Poznámka může existovat úplně bez nich.

Doporučené pravidlo je, že metadata jsou pouze řádky začínající:

```text
@
```

na začátku souboru.

Například:

```text
@title: Produkční server
@aliases: Server, Production Server
@status: active

Produkční server

Server běží na Debianu.

[[Docker]]
[[Backup]]

#server
#production
```

Podporuj minimálně tyto properties:

```text
@title:
@aliases:
@created:
@modified:
@tags:
```

Aplikace však musí být schopná pracovat i s libovolnými uživatelskými properties:

```text
@customer:
@project:
@priority:
@url:
```

Properties nesmí být nutné pro fungování poznámky.

---

## Aliasy poznámek

Poznámka může mít alternativní názvy:

```text
@aliases: Docker Engine, Containers
```

Odkaz:

```text
[[Docker Engine]]
```

se potom může resolverem přeložit na:

```text
Docker.txt
```

Pokud existuje více poznámek se stejným aliasem, aplikace musí nabídnout uživateli výběr.

---

## Komentáře aplikace

Pokud aplikace potřebuje uložit technickou informaci přímo do textu, používej syntaxi:

```text
%% technický komentář %%
```

Tyto komentáře mohou být v běžném editoru viditelné, ale aplikace je může zobrazovat méně výrazně nebo skrýt.

Do těchto komentářů však neukládej kritická metadata aplikace.

Primární technická metadata patří do:

```text
.txtnote/
```

---

## Checkboxy / úkoly

Podporuj jednoduché tasky bez Markdownu:

```text
[ ] Nehotový úkol
[x] Hotový úkol
```

Příklad:

```text
[ ] Aktualizovat server
[x] Vytvořit zálohu
[ ] Zkontrolovat Mautic cron
```

Volitelně může být podporována priorita:

```text
[!] Důležitý úkol
```

nebo datum:

```text
[ ] Aktualizovat server @due:2026-08-15
```

Task management není nutný pro první verzi, ale parser by měl být navržen tak, aby tuto syntaxi bylo možné později doplnit.

---

## Seznamy

Používej jednoduchou plain-text syntaxi:

```text
- první položka
- druhá položka
- třetí položka
```

Číslovaný seznam:

```text
1. první
2. druhý
3. třetí
```

Aplikace je nemusí renderovat jako Markdown.

Editor může pouze nabídnout automatické pokračování seznamu.

---

## Zvýraznění textu

Protože základním principem není Markdown, formátování musí být minimální.

MVP může být čistý plain-text editor bez vizuálního formátování.

Pokud bude později podporováno zvýraznění, použij vlastní jednoduchou syntaxi.

Například:

```text
**důležité**
```

by ale mohlo být zaměňováno za Markdown, proto preferuj, aby první verze žádné inline formátování nevyžadovala.

Prioritou je obsah, odkazy, tagy a struktura, ne rich-text formátování.

---

## Automatická detekce URL

Běžné odkazy:

```text
https://example.com
```

musí být automaticky rozpoznány jako externí URL.

Kliknutí je otevře ve výchozím prohlížeči.

URL se nesmí převádět na jiný formát.

---

## Přílohy

Samotné poznámky jsou vždy `.txt`.

Přílohy mohou být jakékoli běžné soubory, například:

```text
PDF
PNG
JPG
ZIP
DOCX
```

Doporučená struktura:

```text
Notes/
├── .txtnote/
├── attachments/
│   ├── architecture.png
│   └── invoice.pdf
├── Docker.txt
└── Server.txt
```

Odkaz na přílohu může používat:

```text
[[attachments/architecture.png]]
```

Aplikace musí poznat, že cílem není poznámka, ale soubor.

Přípona se u příloh uvádí.

Například:

```text
[[attachments/manual.pdf]]
```

---

## Obrázky

Protože poznámka je čistý text, obrázek není vložen přímo do souboru.

V textu je pouze odkaz:

```text
[[attachments/server-rack.jpg]]
```

UI může obrázek zobrazit jako preview, ale samotný `.txt` soubor stále obsahuje pouze textový odkaz.

---

## Embedy poznámek

Později může být podporováno vložení obsahu jiné poznámky pomocí:

```text
![[Docker]]
```

Aplikace může v preview zobrazit obsah `Docker.txt`.

Samotný soubor ale zůstává:

```text
![[Docker]]
```

Tato funkce není nutná pro MVP.

---

## Konflikty názvů

Je možné, že existují:

```text
Servers/Docker.txt
Projects/Docker.txt
```

Odkaz:

```text
[[Docker]]
```

je v takovém případě nejednoznačný.

Resolver musí použít tento postup:

1. hledat poznámku ve stejné složce,
2. hledat jednoznačnou poznámku v celém workspace,
3. hledat alias,
4. pokud existuje více kandidátů, zobrazit výběr.

Uživatel může vždy použít explicitní cestu:

```text
[[Projects/Docker]]
```

---

## Přejmenování poznámek

Pokud uživatel přejmenuje:

```text
Docker.txt
```

na:

```text
Docker Engine.txt
```

aplikace musí nabídnout automatickou aktualizaci všech wiki odkazů.

Například:

```text
[[Docker]]
```

změnit na:

```text
[[Docker Engine]]
```

Pokud se odkazy neaktualizují, může být původní název automaticky přidán jako alias:

```text
@aliases: Docker
```

To může být konfigurovatelné.

---

## Přesouvání poznámek

Při přesunutí:

```text
Docker.txt
```

do:

```text
Infrastructure/Docker.txt
```

aplikace musí zajistit, že existující odkazy nezůstanou rozbité.

Pokud byl používán pouze:

```text
[[Docker]]
```

a název je v workspace jednoznačný, není změna nutná.

Pokud byly použity explicitní cesty, nabídni jejich automatickou aktualizaci.

---

## Smazané nebo neexistující odkazy

Pokud poznámka obsahuje:

```text
[[Kubernetes]]
```

a:

```text
Kubernetes.txt
```

neexistuje, odkaz je stále validní jako unresolved link.

UI jej musí vizuálně odlišit.

Kliknutí nabídne vytvoření:

```text
Kubernetes.txt
```

Tím lze vytvářet odkazy na poznámky ještě před jejich vznikem.

---

## Encoding a konce řádků

Standardní encoding:

```text
UTF-8
```

Preferovaně bez BOM.

Aplikace musí správně podporovat:

* češtinu,
* diakritiku,
* Unicode,
* emoji,
* další jazyky.

Musí umět načíst:

```text
LF
CRLF
```

Při uložení může zachovat původní line ending daného souboru.

---

## Externí úpravy

Parser nesmí předpokládat, že `.txt` soubory upravuje pouze tato aplikace.

Uživatel je může měnit například pomocí:

```text
Vim
VS Code
Notepad
nano
shell skriptů
Pythonu
Git
Syncthingu
```

Aplikace proto nesmí vyžadovat skrytý stav databáze pro interpretaci samotné poznámky.

Všechny důležité vazby musí být rekonstruovatelné z textových souborů.

---

## Index

Index může obsahovat například:

```text
note ID
cestu
název
aliasy
tagy
wiki odkazy
backlinky
fulltext
mtime
hash souboru
```

Index je však pouze cache.

Pokud bude:

```text
.txtnote/index.db
```

smazán, aplikace musí být schopná celý index znovu vytvořit pouze z obsahu workspace.

---

## Hlavní designové pravidlo formátu

Při každém návrhu nové syntaxe dodržuj toto pravidlo:

> Poznámka musí zůstat dobře čitelná jako obyčejný `.txt` soubor i v případě, že uživatel tuto aplikaci už nikdy nespustí.

Nepřidávej do poznámek velké množství interního markup kódu.

Preferuj jednoduchou, lidsky čitelnou syntaxi.

Formát musí být snadno zpracovatelný také pomocí:

```text
grep
ripgrep
awk
sed
Python
shell skriptů
Git
```

## Doporučený kompletní příklad

```text
@title: Produkční Mautic server
@aliases: Mautic Production, Production Mautic
@created: 2026-08-10
@status: active

Produkční Mautic server

Hlavní server pro provoz Mauticu.

== Infrastruktura ==

Operační systém:
[[Debian]]

Webserver:
[[Apache]]

Databáze:
[[MariaDB]]

Deployment používá:
[[Docker]]

Více informací viz:
[[Docker#Networking|Docker networking]]

== Úkoly ==

[ ] Aktualizovat Mautic
[ ] Zkontrolovat zálohy
[x] Nastavit monitoring

== Dokumentace ==

[[attachments/mautic-architecture.pdf]]

https://mautic.org

#mautic
#server
#production
```

Tento soubor musí být plně použitelný jako obyčejný text i mimo aplikaci.

---

## Upřesnění pro parser

Tato pravidla doplňují nejednoznačná místa výše. Implementuje je
`crates/textura-core`, každé má odpovídající test v
`crates/textura-core/tests/parser.rs`.

### Konec bloku properties

Blok properties začíná na prvním řádku souboru (po případném BOM) a tvoří ho
souvislá řada řádků ve tvaru `@klic: hodnota`.

Blok končí **prvním řádkem, který tomuto tvaru neodpovídá** — včetně prázdného
řádku. Vše za ním je tělo poznámky.

Důsledek: `@` řádek uprostřed textu už property není.

```text
@title: Docker

@tohle už je běžný text
```

Klíč se skládá z alfanumerických znaků, `_`, `-` a `.`, porovnává se
case-insensitive. Hodnota může být prázdná.

### `@` uvnitř řádku

`@klic:hodnota` uvnitř řádku není property poznámky, ale **atribut úkolu**:

```text
[ ] Aktualizovat server @due:2026-08-15
```

Pravidla se liší záměrně: property je celý řádek na začátku souboru a má za
dvojtečkou mezeru, atribut je token uvnitř řádku bez mezery.

### Nadpis sekce

Nadpisem je řádek, na kterém kromě `==`, textu nadpisu a případného `^id` není
nic dalšího. Odsazení je povolené.

```text
== Instalace ==
== Instalace == ^docker-install
=== Vnořená sekce ===
```

Nadpisem naopak není:

```text
V configu je a == b a nic víc.
== ==
==
```

Počet `=` určuje úroveň zanoření (`==` je úroveň 1). Text nadpisu nesmí
obsahovat `=`.

### Tag

`#` musí být na začátku řádku nebo po whitespace či otevírací závorce
a první znak za `#` musí být písmeno (Unicode). Další znaky jsou alfanumerické,
`_`, `-` a `/`. Koncové `/`, `-`, `_` se do tagu nepočítají.

Tagem tedy není `C#`, `#123` ani fragment URL `https://example.com#kotva`.
Tagy se neparsují uvnitř wiki odkazů, URL a komentářů `%% %%`.

### Odkaz vs. příloha

Cíl odkazu je příloha, pokud poslední část cesty končí příponou, která má
1–10 ASCII alfanumerických znaků, obsahuje aspoň jedno písmeno a není `txt`.

* `[[Verze 1.2]]` → poznámka (`.2` není přípona)
* `[[Docker.txt]]` → poznámka, `.txt` se z cíle odstraní
* `[[attachments/manual.pdf]]` → příloha

Odkaz nesmí přesahovat přes konec řádku; neuzavřené `[[` se ignoruje.

### Nezměnitelnost textu

Parser text poznámky nikdy nepřepisuje ani nenormalizuje. Všechny výsledky jsou
byte rozsahy (`Span`) do původního vstupu, včetně BOM, CRLF, diakritiky a emoji.
