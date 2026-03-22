#!/usr/bin/env bash
# ══════════════════════════════════════════════════════════════════════════════
#  Nova installer
#  Usage: ./install.sh [--release]
# ══════════════════════════════════════════════════════════════════════════════
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_NAME="nova"
INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.config/novashell"
SERVICE_DIR="${HOME}/.config/systemd/user"
WIDGETS_SRC="${SCRIPT_DIR}/widgets"
DEFAULT_CFG="${SCRIPT_DIR}/default_config"

# Parse flags
RELEASE_FLAG=""
if [[ "${1:-}" == "--release" ]]; then
    RELEASE_FLAG="--release"
fi

log() { printf "\033[1;34m::\033[0m %s\n" "$*"; }
ok()  { printf "\033[1;32m✔\033[0m  %s\n" "$*"; }
warn(){ printf "\033[1;33m⚠\033[0m  %s\n" "$*"; }

# ── 1. Build ─────────────────────────────────────────────────────────────────
log "Building Nova${RELEASE_FLAG:+ (release)}…"
cd "$SCRIPT_DIR"
cargo build $RELEASE_FLAG 2>&1

BINARY="${SCRIPT_DIR}/target/${RELEASE_FLAG:+release}${RELEASE_FLAG:-debug}/${BINARY_NAME}"

ok "Build complete"

# ── 2. Install binary ─────────────────────────────────────────────────────────
log "Installing binary to ${INSTALL_DIR}…"
mkdir -p "$INSTALL_DIR"
install -m 755 "$BINARY" "${INSTALL_DIR}/${BINARY_NAME}"
ok "Binary installed to ${INSTALL_DIR}/${BINARY_NAME}"

# Warn if INSTALL_DIR not on PATH
if ! echo "$PATH" | tr ':' '\n' | grep -qxF "$INSTALL_DIR"; then
    warn "${INSTALL_DIR} is not in \$PATH — add it to your shell profile"
fi

# ── 3. Create config directory ───────────────────────────────────────────────
log "Setting up config directory at ${CONFIG_DIR}…"
mkdir -p "${CONFIG_DIR}/widgets/custom"
mkdir -p "${CONFIG_DIR}/plugins"
mkdir -p "${CONFIG_DIR}/themes"

# Install default config (don't overwrite existing user config)
if [[ ! -f "${CONFIG_DIR}/config.yaml" ]]; then
    cp "${DEFAULT_CFG}/config.yaml" "${CONFIG_DIR}/config.yaml"
    ok "Installed default config.yaml"
else
    warn "config.yaml already exists — skipping (backup: config.yaml.bak)"
    cp "${CONFIG_DIR}/config.yaml" "${CONFIG_DIR}/config.yaml.bak"
fi

if [[ ! -f "${CONFIG_DIR}/style.css" ]]; then
    cp "${DEFAULT_CFG}/style.css" "${CONFIG_DIR}/style.css"
    ok "Installed default style.css"
else
    warn "style.css already exists — skipping"
fi

# Always sync themes (safe to overwrite — user edits go in style.css)
log "Syncing themes to ${CONFIG_DIR}/themes/…"
for theme_file in "${DEFAULT_CFG}/themes/"*.css; do
    cp "$theme_file" "${CONFIG_DIR}/themes/$(basename "${theme_file}")"
    ok "  + themes/$(basename "${theme_file}")"
done

# Install bundled widget definitions
log "Installing bundled widgets to ${CONFIG_DIR}/widgets/…"
for widget_file in "${WIDGETS_SRC}"/*.widget; do
    dest="${CONFIG_DIR}/widgets/$(basename "${widget_file}")"
    if [[ ! -f "$dest" ]]; then
        cp "$widget_file" "$dest"
        ok "  + $(basename "${widget_file}")"
    else
        warn "  $(basename "${widget_file}") already exists, skipping"
    fi
done

# Install custom widgets
for widget_file in "${WIDGETS_SRC}/custom/"*.widget; do
    dest="${CONFIG_DIR}/widgets/custom/$(basename "${widget_file}")"
    if [[ ! -f "$dest" ]]; then
        cp "$widget_file" "$dest"
        ok "  + custom/$(basename "${widget_file}")"
    else
        warn "  custom/$(basename "${widget_file}") already exists, skipping"
    fi
done

# ── 4. Install systemd user service ──────────────────────────────────────────
log "Installing systemd user service…"
mkdir -p "$SERVICE_DIR"
cp "${SCRIPT_DIR}/nova.service" "${SERVICE_DIR}/nova.service"
systemctl --user daemon-reload 2>/dev/null || true
ok "Service installed. To enable: systemctl --user enable --now nova"

# ── 5. Done ───────────────────────────────────────────────────────────────────
echo ""
echo "══════════════════════════════════════════════════════"
echo "  Nova installed successfully!"
echo ""
echo "  Config:    ${CONFIG_DIR}/config.yaml"
echo "  Styles:    ${CONFIG_DIR}/style.css"
echo "  Themes:    ${CONFIG_DIR}/themes/*.css"
echo ""
echo "  Start:     nova daemon"
echo "  Reload:    nova reload"
echo "  CSS only:  nova reload-css"
echo "  Toggle:    nova toggle <id>"
echo "  List:      nova list"
echo "  Quit:      nova quit"
echo ""
echo "  Apply a theme:"
echo "    cp ${CONFIG_DIR}/themes/tokyo-night.css ${CONFIG_DIR}/style.css"
echo "    nova reload-css"
echo "══════════════════════════════════════════════════════"
