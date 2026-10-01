#!/usr/bin/env bash
#
# install-omarchy.sh — idempotent installer for JSON Viewer on Omarchy Linux.
#
# Removes any previously installed JSON Viewer and installs the current build
# so that it shows up in the Omarchy application menu (Super+Space).
#
# The .desktop entry and icons are registered by the app's own `--install` mode
# (src/desktop.rs), which also runs automatically on every launch. Delegating to
# it keeps a single source of truth for the desktop entry contents, so this
# script and the running app can never fight over the file.
#
# Safe to re-run: every step is idempotent and overwrites the previous install.
#
# Usage:
#   ./install-omarchy.sh              remove old install, build release, install
#   ./install-omarchy.sh --no-build   skip the build, use target/release/jsonviewer
#   ./install-omarchy.sh --run        launch the app once installed
#   ./install-omarchy.sh --uninstall  remove the installation and exit
#   ./install-omarchy.sh --help       show this help

set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"

APP_NAME="jsonviewer"
BIN_SRC="target/release/${APP_NAME}"

DO_BUILD=1
DO_RUN=0
UNINSTALL_ONLY=0

# --- XDG locations ------------------------------------------------------------

BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
BIN_PATH="$BIN_DIR/$APP_NAME"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
APPS_DIR="$DATA_DIR/applications"
DESKTOP_FILE="$APPS_DIR/${APP_NAME}.desktop"
ICON_DIR="$DATA_DIR/icons"
LEGACY_ICON_DIR="$HOME/.icons"

# User-level icons, mirroring the list in src/desktop.rs.
USER_ICONS=(
  "$ICON_DIR/hicolor/512x512/apps/${APP_NAME}.png"
  "$ICON_DIR/hicolor/256x256/apps/${APP_NAME}.png"
  "$ICON_DIR/hicolor/128x128/apps/${APP_NAME}.png"
  "$ICON_DIR/${APP_NAME}.png"
  "$DATA_DIR/pixmaps/${APP_NAME}.png"
  "$LEGACY_ICON_DIR/${APP_NAME}.png"
)

# System-wide locations, as written by `make install`.
SYS_BIN="/usr/local/bin/${APP_NAME}"
SYS_DESKTOP="/usr/share/applications/${APP_NAME}.desktop"
SYS_ICONS=(
  "/usr/share/icons/hicolor/512x512/apps/${APP_NAME}.png"
  "/usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png"
  "/usr/share/icons/hicolor/128x128/apps/${APP_NAME}.png"
  "/usr/share/pixmaps/${APP_NAME}.png"
)

# --- Output helpers -----------------------------------------------------------

if [[ -t 1 ]]; then
  B=$'\033[1m'; G=$'\033[32m'; Y=$'\033[33m'; R=$'\033[31m'; N=$'\033[0m'
else
  B=""; G=""; Y=""; N=""; R=""
fi

step() { printf '%s==>%s %s\n' "$B" "$N" "$*"; }
ok()   { printf '    %s✓%s %s\n' "$G" "$N" "$*"; }
warn() { printf '    %s!%s %s\n' "$Y" "$N" "$*"; }
err()  { printf '    %sx%s %s\n' "$R" "$N" "$*" >&2; }

have() { command -v "$1" >/dev/null 2>&1; }

usage() {
  cat <<'USAGE'
install-omarchy.sh — idempotent installer for JSON Viewer on Omarchy Linux.

Removes any previously installed JSON Viewer and installs the current build
so that it appears in the Omarchy application menu.

Usage:
  ./install-omarchy.sh              remove old install, build release, install
  ./install-omarchy.sh --no-build   skip the build, use target/release/jsonviewer
  ./install-omarchy.sh --run        launch the app once installed
  ./install-omarchy.sh --uninstall  remove the installation and exit
  ./install-omarchy.sh --help       show this help
USAGE
}

# --- Argument parsing ---------------------------------------------------------

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-build)  DO_BUILD=0 ;;
    --run)       DO_RUN=1 ;;
    --uninstall) UNINSTALL_ONLY=1 ;;
    -h|--help)   usage; exit 0 ;;
    *)           err "unknown option: $1"; echo; usage; exit 2 ;;
  esac
  shift
done

# --- Removal ------------------------------------------------------------------

stop_running() {
  if pgrep -x "$APP_NAME" >/dev/null 2>&1; then
    pkill -x "$APP_NAME" 2>/dev/null || true
    sleep 1
    pkill -9 -x "$APP_NAME" 2>/dev/null || true
    ok "stopped running ${APP_NAME}"
  fi
}

remove_user_install() {
  local found=0

  if [[ -e "$BIN_PATH" ]]; then
    rm -f "$BIN_PATH"; found=1; ok "removed $BIN_PATH"
  fi
  if [[ -e "$DESKTOP_FILE" ]]; then
    rm -f "$DESKTOP_FILE"; found=1; ok "removed $DESKTOP_FILE"
  fi
  for icon in "${USER_ICONS[@]}"; do
    if [[ -e "$icon" ]]; then
      rm -f "$icon"; found=1; ok "removed $icon"
    fi
  done

  [[ $found -eq 0 ]] && ok "no previous user-level install found"
  return 0
}

remove_system_install() {
  local found=0 f

  for f in "$SYS_BIN" "$SYS_DESKTOP" "${SYS_ICONS[@]}"; do
    [[ -e "$f" ]] && found=1
  done

  if [[ $found -eq 0 ]]; then
    ok "no previous system-wide install found"
    return 0
  fi

  # A system-wide copy would shadow the new one on $PATH and in the menu.
  if [[ ! -t 0 ]]; then
    warn "system-wide install found but not interactive; run with sudo to remove:"
    for f in "$SYS_BIN" "$SYS_DESKTOP" "${SYS_ICONS[@]}"; do
      [[ -e "$f" ]] && printf '      %s\n' "$f"
    done
    return 0
  fi

  for f in "$SYS_BIN" "$SYS_DESKTOP" "${SYS_ICONS[@]}"; do
    if [[ -e "$f" ]]; then
      if sudo rm -f "$f"; then ok "removed $f"; else warn "could not remove $f"; fi
    fi
  done
}

# --- Build & install ----------------------------------------------------------

do_build() {
  step "Building ${APP_NAME} (release)"
  if ! have cargo; then
    err "cargo not found — install Rust (https://rustup.rs) or pass --no-build"
    exit 1
  fi
  cargo build --release
}

do_install() {
  step "Installing to $BIN_PATH"
  mkdir -p "$BIN_DIR"
  # Stage then rename so replacing a running binary never hits ETXTBSY.
  install -m 755 "$BIN_SRC" "${BIN_PATH}.new"
  mv -f "${BIN_PATH}.new" "$BIN_PATH"
  ok "installed $BIN_PATH"
}

register_desktop() {
  step "Registering desktop entry and icons"
  # The app writes ~/.local/share/applications/<name>.desktop plus every icon
  # path in src/desktop.rs, using its own resolved path for Exec=.
  "$BIN_PATH" --install
}

refresh_caches() {
  step "Refreshing desktop, icon and menu caches"

  if have update-desktop-database; then
    update-desktop-database "$APPS_DIR" >/dev/null 2>&1 \
      && ok "desktop database updated" || warn "update-desktop-database failed"
  fi

  if have gtk-update-icon-cache && [[ -d "$ICON_DIR/hicolor" ]]; then
    gtk-update-icon-cache -f -t "$ICON_DIR/hicolor" >/dev/null 2>&1 \
      && ok "icon cache updated" || warn "gtk-update-icon-cache failed"
  fi

  if have omarchy; then
    omarchy menu refresh >/dev/null 2>&1 \
      && ok "omarchy menu refreshed" || warn "omarchy menu refresh failed"
  fi
}

verify() {
  step "Verifying installation"

  if [[ ! -x "$BIN_PATH" ]]; then
    err "binary missing at $BIN_PATH"; exit 1
  fi

  if [[ -f "$DESKTOP_FILE" ]]; then
    ok "desktop entry: $DESKTOP_FILE"
  else
    err "desktop entry missing: $DESKTOP_FILE"; exit 1
  fi

  local icon_ok=0
  for icon in "${USER_ICONS[@]}"; do
    [[ -f "$icon" ]] && icon_ok=1
  done
  [[ $icon_ok -eq 1 ]] && ok "icons installed" || warn "no icons found"

  if have desktop-file-validate; then
    if desktop-file-validate "$DESKTOP_FILE" 2>/dev/null; then
      ok "desktop entry valid"
    else
      warn "desktop-file-validate reported issues (see above)"
    fi
  fi

  printf '\n%sJSON Viewer installed.%s Launch it from the Omarchy menu (Super+Space)\n' "$G" "$N"
  printf 'and search for %s"JSON Viewer"%s, or run:\n\n    %s [FILE]\n\n' "$B" "$N" "$BIN_PATH"
}

# --- Main ---------------------------------------------------------------------

if [[ $UNINSTALL_ONLY -eq 1 ]]; then
  step "Uninstalling JSON Viewer"
  stop_running
  remove_user_install
  remove_system_install
  refresh_caches
  printf '\n%JSON Viewer uninstalled.%s\n' "$G" "$N"
  exit 0
fi

step "Installing JSON Viewer for the current user"
stop_running
remove_user_install
remove_system_install

if [[ $DO_BUILD -eq 1 ]]; then
  do_build
else
  step "Skipping build (--no-build)"
  [[ -x "$BIN_SRC" ]] || { err "$BIN_SRC not found; drop --no-build"; exit 1; }
fi

do_install
register_desktop
refresh_caches
verify

if [[ $DO_RUN -eq 1 ]]; then
  step "Launching JSON Viewer"
  setsid "$BIN_PATH" >/dev/null 2>&1 </dev/null &
  ok "launched"
fi