#!/usr/bin/env bash
#
# Postaví macOS balíček Textury (.dmg) a volitelně ho přidá k existujícímu
# GitHub releasu.
#
#   ./release-macos.sh              postaví .dmg z aktuálního stromu
#   ./release-macos.sh --upload     navíc ho přiloží k releasu podle verze
#   ./release-macos.sh --dry-run    jen ukáže, co by se stalo
#
# Verzi ani tag tenhle skript nemění — od toho je ./release.sh, který vydává
# z Linuxu. Tady se jen doplní balíček pro macOS k verzi, která už existuje.
# Musí běžet na macOS: křížový build Tauri aplikace z Linuxu není možný.

set -euo pipefail

UPLOAD=0
DRY_RUN=0

die() {
    printf '\033[31mchyba:\033[0m %s\n' "$1" >&2
    exit 1
}

step() {
    printf '\n\033[1;34m▸ %s\033[0m\n' "$1"
}

usage() {
    sed -n '3,12p' "$0" | sed 's/^# \?//'
    exit "${1:-0}"
}

while [ $# -gt 0 ]; do
    case "$1" in
        --upload) UPLOAD=1 ;;
        --dry-run) DRY_RUN=1 ;;
        -h | --help) usage 0 ;;
        *) die "neznámý přepínač: $1" ;;
    esac
    shift
done

# Rustup se instaloval bez zásahu do PATH, tak si cargo najdeme sami.
if ! command -v cargo >/dev/null 2>&1; then
    # shellcheck source=/dev/null
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
fi

# --- kontroly --------------------------------------------------------------

step "Kontroly"

[ "$(uname -s)" = "Darwin" ] || die "tenhle skript patří na macOS (na Linuxu vydává ./release.sh)"

command -v cargo >/dev/null 2>&1 || die "cargo není v PATH (zkus: . \"\$HOME/.cargo/env\")"
command -v cargo-tauri >/dev/null 2>&1 || cargo tauri --version >/dev/null 2>&1 ||
    die "tauri-cli chybí (nainstaluj: cargo install tauri-cli --version '^2' --locked)"
command -v npm >/dev/null 2>&1 || die "npm není v PATH"
xcode-select -p >/dev/null 2>&1 || die "chybí nástroje Xcode (spusť: xcode-select --install)"

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

VERSION="$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)"
[ -n "$VERSION" ] || die "verzi se nepodařilo přečíst z Cargo.toml"
TAG="v$VERSION"

# Apple Silicon i Intel — buduje se pro architekturu tohohle stroje.
ARCH="$(uname -m)"
case "$ARCH" in
    arm64) TARGET="aarch64-apple-darwin" ;;
    x86_64) TARGET="x86_64-apple-darwin" ;;
    *) die "neznámá architektura: $ARCH" ;;
esac

DMG="target/$TARGET/release/bundle/dmg/Textura_${VERSION}_${ARCH}.dmg"

echo "  verze:        $VERSION"
echo "  architektura: $ARCH ($TARGET)"

if [ "$UPLOAD" = 1 ]; then
    command -v gh >/dev/null 2>&1 || die "gh není v PATH (balíček by se neměl kam přiložit)"
    gh auth status >/dev/null 2>&1 || die "gh není přihlášené (spusť: gh auth login)"
    gh release view "$TAG" >/dev/null 2>&1 ||
        die "release $TAG na GitHubu není — vydej ho nejdřív z Linuxu přes ./release.sh"
fi

if [ "$DRY_RUN" = 1 ]; then
    step "Zkušební běh — dál už se nic nemění"
    echo "  udělalo by se:"
    echo "    1. cargo test"
    echo "    2. npm --prefix ui install"
    echo "    3. cargo tauri build --target $TARGET (.app + .dmg)"
    [ "$UPLOAD" = 1 ] && echo "    4. gh release upload $TAG $(basename "$DMG")"
    exit 0
fi

# --- testy a build ---------------------------------------------------------

step "Testy"
cargo test

step "Balíček"
npm --prefix ui install --silent
rustup target add "$TARGET" >/dev/null 2>&1 || true
(cd crates/textura-app && cargo tauri build --target "$TARGET")

[ -f "$DMG" ] || die "balíček nevznikl: $DMG"

# --- upload ----------------------------------------------------------------

if [ "$UPLOAD" = 1 ]; then
    step "Přikládám k releasu $TAG"
    gh release upload "$TAG" "$DMG" --clobber
fi

step "Hotovo: $(basename "$DMG")"
echo "  $DMG"
echo
echo "  Balíček není podepsaný Apple certifikátem, takže po prvním otevření"
echo "  Gatekeeper zamručí. Uživateli stačí:"
echo "    xattr -dr com.apple.quarantine /Applications/Textura.app"
