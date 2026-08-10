#!/usr/bin/env bash
#
# Vydá novou verzi Textury.
#
#   ./release.sh 0.2.0              vydá verzi 0.2.0
#   ./release.sh 0.2.0 --dry-run    jen ukáže, co by se stalo
#   ./release.sh 0.2.0 --yes        nezeptá se před pushem
#
# Postup: kontroly → zvýšení verze → testy → balíčky → commit → tag → push →
# GitHub release. Cokoli selže, skript končí a nic dalšího neprovede.

set -euo pipefail

VERSION=""
DRY_RUN=0
ASSUME_YES=0

die() {
    printf '\033[31mchyba:\033[0m %s\n' "$1" >&2
    exit 1
}

step() {
    printf '\n\033[1;34m▸ %s\033[0m\n' "$1"
}

usage() {
    sed -n '3,10p' "$0" | sed 's/^# \?//'
    exit "${1:-0}"
}

# --- argumenty -------------------------------------------------------------

while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=1 ;;
        --yes | -y) ASSUME_YES=1 ;;
        -h | --help) usage 0 ;;
        -*) die "neznámý přepínač: $1" ;;
        *)
            [ -z "$VERSION" ] || die "verzi zadej jen jednou"
            VERSION="$1"
            ;;
    esac
    shift
done

[ -n "$VERSION" ] || usage 1
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "verze musí být ve tvaru X.Y.Z, dostal jsem '$VERSION'"

TAG="v$VERSION"

# Rustup se instaloval bez zásahu do PATH, tak si cargo najdeme sami.
if ! command -v cargo >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
fi

# --- kontroly před zásahem -------------------------------------------------

step "Kontroly"

command -v cargo >/dev/null 2>&1 || die "cargo není v PATH (zkus: . \"\$HOME/.cargo/env\")"
command -v cargo-tauri >/dev/null 2>&1 || cargo tauri --version >/dev/null 2>&1 ||
    die "tauri-cli chybí (nainstaluj: cargo install tauri-cli --version '^2' --locked)"
command -v npm >/dev/null 2>&1 || die "npm není v PATH"
command -v gh >/dev/null 2>&1 || die "gh není v PATH (release by se nezaložil)"

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
[ "$BRANCH" = "main" ] || die "jsi na větvi '$BRANCH', vydává se z main"

[ -z "$(git status --porcelain)" ] || die "pracovní strom není čistý — commitni nebo zahoď změny"

if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
    # Tag jen lokálně znamená přerušený běh — poraď, jak ho dokončit,
    # ať uživatel nemusí hádat.
    if [ -z "$(git ls-remote --tags origin "refs/tags/$TAG" 2>/dev/null)" ]; then
        printf '\033[33mtag %s existuje lokálně, ale ne na originu — vypadá to na přerušené vydání.\033[0m\n' "$TAG" >&2
        echo "  dokončit:  git push origin main && git push origin $TAG" >&2
        echo "  zahodit:   git tag -d $TAG && git reset --hard HEAD^" >&2
    fi
    die "tag $TAG už existuje"
fi

gh auth status >/dev/null 2>&1 || die "gh není přihlášené (spusť: gh auth login)"

# Ať nevydáváme něco, co na GitHubu chybí nebo je pozadu.
git fetch --quiet origin main
LOCAL="$(git rev-parse main)"
REMOTE="$(git rev-parse origin/main 2>/dev/null || echo "")"
if [ -n "$REMOTE" ] && [ "$LOCAL" != "$REMOTE" ]; then
    git merge-base --is-ancestor "$REMOTE" "$LOCAL" ||
        die "origin/main je napřed — nejdřív git pull"
fi

CURRENT="$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)"
echo "  aktuální verze: $CURRENT"
echo "  nová verze:     $VERSION"
[ "$CURRENT" != "$VERSION" ] || die "verze $VERSION už je nastavená v Cargo.toml"

PREV_TAG="$(git describe --tags --abbrev=0 2>/dev/null || echo "")"

if [ "$DRY_RUN" = 1 ]; then
    step "Zkušební běh — dál už se nic nemění"
    echo "  udělalo by se:"
    echo "    1. verze v Cargo.toml → $VERSION"
    echo "    2. cargo test"
    echo "    3. cargo tauri build (deb, rpm, AppImage)"
    echo "    4. commit \"Verze $VERSION\" + tag $TAG"
    echo "    5. push main a $TAG na origin"
    echo "    6. gh release create $TAG s balíčky"
    [ -n "$PREV_TAG" ] && {
        echo
        echo "  commity od $PREV_TAG, ze kterých vznikne popis tagu:"
        git log --pretty='    - %s' "$PREV_TAG..HEAD"
    }
    exit 0
fi

# --- verze -----------------------------------------------------------------
#
# tauri.conf.json verzi záměrně neobsahuje — Tauri ji bere z Cargo.toml,
# takže je na jednom místě.

step "Zvyšuji verzi na $VERSION"
sed -i "s/^version = \"$CURRENT\"$/version = \"$VERSION\"/" Cargo.toml
grep -q "^version = \"$VERSION\"$" Cargo.toml || die "zápis verze do Cargo.toml selhal"

# Aby se nová verze promítla i do Cargo.lock.
cargo metadata --format-version 1 >/dev/null

# Když cokoli dál selže, nenecháme za sebou rozbabranou verzi.
rollback() {
    printf '\n\033[33mvracím změny verze\033[0m\n' >&2
    git checkout -- Cargo.toml Cargo.lock 2>/dev/null || true
}
trap rollback ERR

# --- testy a build ---------------------------------------------------------

step "Testy"
cargo test

step "Balíčky"
npm --prefix ui install --silent
(cd crates/textura-app && cargo tauri build)

DEB="target/release/bundle/deb/Textura_${VERSION}_amd64.deb"
RPM="target/release/bundle/rpm/Textura-${VERSION}-1.x86_64.rpm"
APPIMAGE="target/release/bundle/appimage/Textura_${VERSION}_amd64.AppImage"

for f in "$DEB" "$RPM" "$APPIMAGE"; do
    [ -f "$f" ] || die "balíček nevznikl: $f"
done

# --- commit a tag ----------------------------------------------------------

step "Commit a tag"
git add Cargo.toml Cargo.lock
git commit -q -m "Verze $VERSION"

if [ -n "$PREV_TAG" ]; then
    CHANGES="$(git log --pretty='- %s' "$PREV_TAG..HEAD^")"
else
    CHANGES="$(git log --pretty='- %s' HEAD^)"
fi

git tag -a "$TAG" -F - <<EOF
Textura $VERSION

$CHANGES
EOF

trap - ERR

# --- push a release --------------------------------------------------------

echo
echo "Připraveno k odeslání:"
echo "  commit:  $(git log --oneline -1)"
echo "  tag:     $TAG"
echo "  balíčky: $(basename "$DEB"), $(basename "$RPM"), $(basename "$APPIMAGE")"

if [ "$ASSUME_YES" != 1 ]; then
    printf '\nPushnout a založit release? [a/N] '
    read -r answer
    case "$answer" in
        a | A | y | Y) ;;
        *)
            # Rozdělaný stav je horší než žádný: commit a tag by zůstaly
            # lokálně, nebylo by je vidět a příští běh by se o existující tag
            # zastavil. Vracíme proto všechno do stavu před spuštěním.
            step "Zastaveno — vracím commit i tag"
            git tag -d "$TAG" >/dev/null
            git reset --hard HEAD^ >/dev/null
            echo "  Verze zpátky na $CURRENT, pracovní strom čistý."
            echo "  Balíčky $VERSION zůstávají v target/release/bundle/."
            exit 0
            ;;
    esac
fi

step "Push"
git push origin main
git push origin "$TAG"

step "GitHub release"
# --notes-from-tag nejde kombinovat s --repo, proto běžíme z adresáře repa.
gh release create "$TAG" "$DEB" "$RPM" "$APPIMAGE" \
    --title "Textura $VERSION" \
    --verify-tag \
    --notes-from-tag

step "Hotovo: Textura $VERSION"
echo "  Aktualizace nainstalované aplikace:"
echo "    sudo apt install ./$DEB"
