#!/bin/sh
# Launch Open Wispr at login. Loads GROQ_API_KEY from shell profiles or config.

LOG="${HOME}/.config/open-wisper/autostart.log"
mkdir -p "${HOME}/.config/open-wisper"

{
    echo "$(date -Iseconds) starting open-wisper-tray"

    for f in "${HOME}/.profile" "${HOME}/.zprofile"; do
        if [ -f "$f" ]; then
            # shellcheck disable=SC1090
            . "$f"
        fi
    done

    # .zshrc often has zsh-only syntax; extract GROQ_API_KEY without sourcing it.
    if [ -z "${GROQ_API_KEY:-}" ]; then
        for f in "${HOME}/.zshrc" "${HOME}/.zprofile" "${HOME}/.profile"; do
            if [ -f "$f" ]; then
                line=$(grep -m1 '^export GROQ_API_KEY=' "$f" 2>/dev/null || true)
                if [ -n "$line" ]; then
                    val="${line#export GROQ_API_KEY=}"
                    val="${val%\"}"
                    val="${val#\"}"
                    val="${val%\'}"
                    val="${val#\'}"
                    export GROQ_API_KEY="$val"
                    break
                fi
            fi
        done
    fi

    if [ -z "${GROQ_API_KEY:-}" ]; then
        echo "error: GROQ_API_KEY not found in profile files or config"
        exit 1
    fi

    exec "${HOME}/.local/bin/nexus-dictation" run
} >>"${LOG}" 2>&1
