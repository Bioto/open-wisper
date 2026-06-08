#!/usr/bin/env bash
# Install Open Wispr to ~/.local/bin and enable login autostart (Linux or macOS).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN_NAME="nexus-dictation"
INSTALL_DIR="${HOME}/.local/bin"
INSTALL_BIN="${INSTALL_DIR}/${BIN_NAME}"
TRAY_SCRIPT="${INSTALL_DIR}/open-wisper-tray"
LAUNCH_AGENT_LABEL="com.open-wisper.dictation"
LAUNCH_AGENT_DIR="${HOME}/Library/LaunchAgents"
LAUNCH_AGENT_PLIST="${LAUNCH_AGENT_DIR}/${LAUNCH_AGENT_LABEL}.plist"
AUTOSTART_DESKTOP="${HOME}/.config/autostart/open-wisper.desktop"
LOG_DIR="${HOME}/Library/Logs"
LOG_FILE="${LOG_DIR}/open-wisper.log"

echo "Building release binary..."
cargo build --release -p nexus_dictation --manifest-path "${REPO_ROOT}/Cargo.toml"

mkdir -p "${INSTALL_DIR}"
cp "${REPO_ROOT}/target/release/${BIN_NAME}" "${INSTALL_BIN}"
chmod +x "${INSTALL_BIN}"

cp "${REPO_ROOT}/scripts/open-wisper-tray.sh" "${TRAY_SCRIPT}"
chmod +x "${TRAY_SCRIPT}"

OS="$(uname -s)"
case "${OS}" in
    Darwin)
        mkdir -p "${LAUNCH_AGENT_DIR}" "${LOG_DIR}"
        cat >"${LAUNCH_AGENT_PLIST}" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${LAUNCH_AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>${TRAY_SCRIPT}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>LimitLoadToSessionType</key>
    <array>
        <string>Aqua</string>
    </array>
    <key>StandardOutPath</key>
    <string>${LOG_FILE}</string>
    <key>StandardErrorPath</key>
    <string>${LOG_FILE}</string>
</dict>
</plist>
EOF

        UID="$(id -u)"
        launchctl bootout "gui/${UID}" "${LAUNCH_AGENT_PLIST}" >/dev/null 2>&1 || true
        launchctl bootstrap "gui/${UID}" "${LAUNCH_AGENT_PLIST}"

        echo "Installed macOS LaunchAgent: ${LAUNCH_AGENT_PLIST}"
        echo "Logs: ${LOG_FILE}"
        echo "Start now: launchctl kickstart -k gui/${UID}/${LAUNCH_AGENT_LABEL}"
        ;;
    Linux)
        mkdir -p "${HOME}/.config/autostart"
        cat >"${AUTOSTART_DESKTOP}" <<EOF
[Desktop Entry]
Type=Application
Name=Open Wispr
Comment=Voice dictation tray app
Exec=${TRAY_SCRIPT}
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
X-GNOME-Autostart-Delay=2
EOF

        if command -v desktop-file-validate >/dev/null 2>&1; then
            desktop-file-validate "${AUTOSTART_DESKTOP}"
        fi

        echo "Installed Linux autostart entry: ${AUTOSTART_DESKTOP}"
        echo "Log out and back in (or reboot) to start automatically."
        ;;
    *)
        echo "Unsupported OS: ${OS}" >&2
        echo "Binary installed to ${INSTALL_BIN}, but autostart was not configured." >&2
        exit 1
        ;;
esac

echo "Binary: ${INSTALL_BIN}"
echo "Ensure GROQ_API_KEY is set in ~/.profile, ~/.zprofile, or config.toml before login."
