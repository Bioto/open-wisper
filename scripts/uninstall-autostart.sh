#!/usr/bin/env bash
# Remove Open Wispr login autostart (Linux or macOS).
set -euo pipefail

LAUNCH_AGENT_LABEL="com.open-wisper.dictation"
LAUNCH_AGENT_PLIST="${HOME}/Library/LaunchAgents/${LAUNCH_AGENT_LABEL}.plist"
AUTOSTART_DESKTOP="${HOME}/.config/autostart/open-wisper.desktop"

OS="$(uname -s)"
case "${OS}" in
    Darwin)
        UID="$(id -u)"
        if [ -f "${LAUNCH_AGENT_PLIST}" ]; then
            launchctl bootout "gui/${UID}" "${LAUNCH_AGENT_PLIST}" >/dev/null 2>&1 || true
            rm -f "${LAUNCH_AGENT_PLIST}"
            echo "Removed macOS LaunchAgent: ${LAUNCH_AGENT_PLIST}"
        else
            echo "No LaunchAgent found at ${LAUNCH_AGENT_PLIST}"
        fi
        ;;
    Linux)
        if [ -f "${AUTOSTART_DESKTOP}" ]; then
            rm -f "${AUTOSTART_DESKTOP}"
            echo "Removed Linux autostart entry: ${AUTOSTART_DESKTOP}"
        else
            echo "No autostart entry found at ${AUTOSTART_DESKTOP}"
        fi
        ;;
    *)
        echo "Unsupported OS: ${OS}" >&2
        exit 1
        ;;
esac
