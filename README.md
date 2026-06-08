# Open Wispr

An open-source voice dictation toolkit inspired by [Wispr Flow](https://wisprflow.ai/). Speak into any app — Open Wispr transcribes with Groq Whisper, polishes the text with an LLM, and injects it at your cursor.

## Modules

| Module | Description |
|--------|-------------|
| `nexus_dictation` | Voice dictation app: global hotkey, Groq ASR + LLM rewrite, text injection, tray + HUD UI |
| `nexus_recorder` | Desktop screen/audio/input recording with click overlay post-processing |

## Quick Start (Dictation)

```bash
# Install system deps (Linux)
sudo apt install build-essential pkg-config libasound2-dev libxdo-dev libgtk-3-dev libappindicator3-dev

# Set your Groq API key (free tier at https://console.groq.com)
export GROQ_API_KEY=gsk_...

# Build (fast — no local Whisper model)
cargo build --release -p nexus_dictation

# Headless test: record 5 seconds, transcribe, rewrite, print
make dictation-test

# Run the full app (tray only by default)
make dictation
```

Default hotkey: **Alt + Shift + Z** (hold to record, release to transcribe and inject).

Config is written on first run to `~/.config/open-wisper/config.toml`.

Typical latency: **1–3 seconds** end-to-end (Groq ASR + LLM rewrite) vs 15–30s with local CPU Whisper.

## nexus_dictation

Cloud-first voice dictation powered by Groq.

### Features

- **Groq ASR** — `whisper-large-v3-turbo` via Groq transcription API
- **LLM rewrite** — `llama-3.3-70b-versatile` cleans filler words and grammar before paste
- **Global hotkey** — Push-to-talk or toggle mode
- **Text injection** — Type or clipboard-paste into the focused app (Wayland-safe paste mode)
- **Tray + dictation HUD** — Runs from the system tray; center-screen spinner while recording/transcribing; settings pane from tray menu
- **Cross-platform** — Linux (X11), macOS, Windows
- **Optional local ASR** — Build with `--features local-asr` for offline Whisper via whisper.cpp

### CLI

```bash
nexus-dictation run                    # Tray app (default)
nexus-dictation test --seconds 5       # Headless record + transcribe + rewrite
nexus-dictation test --seconds 5 --inject  # Also inject into focused app
nexus-dictation list-devices           # List microphone devices
nexus-dictation show-config            # Print config path and settings

# local-asr feature only:
nexus-dictation download-model         # Fetch ggml-base.en from HuggingFace
```

### Configuration (`~/.config/open-wisper/config.toml`)

```toml
[hotkey]
modifiers = "alt,shift"
key = "KeyZ"
mode = "push_to_talk"  # or "toggle"

[asr]
provider = "groq"
model = "whisper-large-v3-turbo"
language = "en"

[llm]
enabled = true
model = "llama-3.3-70b-versatile"

inject_mode = "paste"  # "type", "paste", or "auto"
paste_threshold = 80
```

### Autostart at login

Install the release binary to `~/.local/bin` and enable platform autostart:

```bash
make install-autostart
```

- **Linux (i3, GNOME, etc.)**: installs `~/.config/autostart/open-wisper.desktop`
- **macOS**: installs `~/Library/LaunchAgents/com.open-wisper.dictation.plist` and loads it with `launchctl`

Remove autostart:

```bash
make uninstall-autostart
```

Ensure `GROQ_API_KEY` is available at login. The tray wrapper reads it from `~/.profile`, `~/.zprofile`, or `~/.zshrc`, or you can set `api_key` in `config.toml`.

**Linux troubleshooting (i3 + dex):**

```bash
# Check autostart log after login
cat ~/.config/open-wisper/autostart.log

# Start manually (same as dex autostart)
~/.local/bin/open-wisper-tray

# Re-run all XDG autostart entries
dex --autostart --environment i3
```

**macOS manual check:**

```bash
launchctl print gui/$(id -u)/com.open-wisper.dictation
tail -f ~/Library/Logs/open-wisper.log
```

### i3 / tiling window managers

If the dictation HUD tiles instead of floating, add to your i3 config:

```
for_window [class="open-wisper"] floating enable
```

API key resolution: `GROQ_API_KEY` environment variable first, then optional `api_key` in `[asr]` or `[llm]`.

Set `llm.enabled = false` to skip LLM rewrite and use basic punctuation cleanup only.

### Optional local Whisper (`local-asr` feature)

For offline dictation without sending audio to Groq:

```bash
sudo apt install cmake  # required by whisper.cpp
cargo build --release -p nexus_dictation --features local-asr
make dictation-model    # downloads ~150 MB ggml-base.en
```

Then set `provider = "local"` under `[asr]` in config.

## nexus_recorder

Desktop recording library with CLI and PyO3 Python bindings.

```bash
cd src/nexus_recorder && uv sync --group dev
./target/release/nexus-recorder --help
```

See [src/nexus_recorder](src/nexus_recorder) for screen/audio/input recording docs.

## Prerequisites

- Rust 1.70+
- **Groq API key** — required for default cloud ASR + LLM path
- **Linux**: ALSA dev headers (`libasound2-dev`), X11 for global hotkeys, GTK for tray
- **macOS**: Xcode command line tools
- **Windows**: Visual Studio Build Tools
- **cmake** — only for optional `local-asr` feature (whisper.cpp)
- FFmpeg (for `nexus_recorder` only)
- Python 3.9+ and [UV](https://github.com/astral-sh/uv) (for recorder Python bindings)

## License

MIT — see [LICENSE](LICENSE).
