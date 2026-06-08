.PHONY: build release test clippy dictation dictation-test dictation-model nexus-recorder-wheel nexus-recorder-wheel-dev nexus-recorder-wheel-all clickhouse-up clickhouse-down clickhouse-logs clickhouse-status clickhouse-restart

build:
	cargo build -p nexus_dictation

release:
	cargo build --release -p nexus_dictation

test:
	cargo test -p nexus_dictation

clippy:
	cargo clippy -p nexus_dictation --all-targets -- -D warnings

# Run Open Wispr dictation app (tray + overlay)
dictation:
	cargo run --release -p nexus_dictation -- run

# Headless dictation test: record 5s, transcribe, print
dictation-test:
	cargo run --release -p nexus_dictation -- test --seconds 5

# Download local Whisper ggml model (requires --features local-asr)
dictation-model:
	cargo run --release -p nexus_dictation --features local-asr -- download-model

# Nexus Recorder wheel build commands
nexus-recorder-wheel:
	@if command -v maturin >/dev/null 2>&1; then \
		cd src/nexus_recorder && maturin build --release; \
	elif command -v uv >/dev/null 2>&1; then \
		cd src/nexus_recorder && uv pip install maturin && uv run maturin build --release; \
	else \
		echo "Error: maturin not found. Install with: cargo install maturin"; \
		exit 1; \
	fi

nexus-recorder-wheel-all:
	@set -e; \
	if command -v maturin >/dev/null 2>&1; then \
		MATURIN_CMD="maturin"; \
	elif command -v uv >/dev/null 2>&1; then \
		cd src/nexus_recorder && uv pip install maturin; \
		MATURIN_CMD="uv run maturin"; \
	else \
		echo "Error: maturin not found. Install with: cargo install maturin"; \
		exit 1; \
	fi; \
	UNAME_S=$$(uname -s); \
	if [ "$$UNAME_S" = "Linux" ]; then \
	  rustup target add x86_64-pc-windows-gnu; \
	  (cd src/nexus_recorder && $$MATURIN_CMD build --release); \
	  (cd src/nexus_recorder && $$MATURIN_CMD build --release --target x86_64-pc-windows-gnu); \
	elif [ "$$UNAME_S" = "Darwin" ]; then \
	  rustup target add x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu; \
	  (cd src/nexus_recorder && $$MATURIN_CMD build --release); \
	  (cd src/nexus_recorder && $$MATURIN_CMD build --release --target x86_64-apple-darwin); \
	  (cd src/nexus_recorder && $$MATURIN_CMD build --release --target aarch64-apple-darwin); \
	  (cd src/nexus_recorder && $$MATURIN_CMD build --release --target x86_64-pc-windows-gnu); \
	else \
	  (cd src/nexus_recorder && $$MATURIN_CMD build --release); \
	fi

nexus-recorder-wheel-dev:
	@if command -v maturin >/dev/null 2>&1; then \
		cd src/nexus_recorder && maturin build; \
	elif command -v uv >/dev/null 2>&1; then \
		cd src/nexus_recorder && uv pip install maturin && uv run maturin build; \
	else \
		echo "Error: maturin not found. Install with: cargo install maturin"; \
		exit 1; \
	fi

# ClickHouse (optional analytics infra)
clickhouse-up:
	docker compose -f .docker/clickhouse/docker-compose.yaml up -d

clickhouse-down:
	docker compose -f .docker/clickhouse/docker-compose.yaml down

clickhouse-logs:
	docker compose -f .docker/clickhouse/docker-compose.yaml logs -f

clickhouse-status:
	docker compose -f .docker/clickhouse/docker-compose.yaml ps

clickhouse-restart:
	docker compose -f .docker/clickhouse/docker-compose.yaml restart
