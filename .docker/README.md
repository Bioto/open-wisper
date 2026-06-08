# Docker Configuration

This directory contains Docker configurations for the nexus project.

## Directory Structure

```
.docker/
├── README.md           # This file
├── nexus/              # Main nexus build
│   └── Dockerfile
└── clickhouse/         # ClickHouse analytics (optional)
    ├── docker-compose.yaml
    └── config/
        └── cors.xml
```

## Stacks

### 1. Nexus (Main Build)

Full workspace build for the nexus project.

```bash
# Build from workspace root
docker build -f .docker/nexus/Dockerfile -t nexus .

# Run
docker run --rm nexus --help
```

### 2. ClickHouse (Optional)

Analytics database with web UI.

```bash
cd .docker/clickhouse
docker compose up -d
```

**Endpoints:**
- HTTP interface: `http://localhost:8123`
- Native protocol: `localhost:9000`
