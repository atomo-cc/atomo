# Configuration

## Environment
- Copy `.env.example` to `.env` in repo root or service dir.
- Common vars: `DATABASE_URL`, `RUST_LOG`.

## Service Config
- Each service includes metadata in `package.json` under `atomo`:
```json
{
  "atomo": {
    "service": "crm",
    "configFile": "./atomo.config.ts",
    "schemaFile": "./schema.ts",
    "pluginsDir": "./plugins",
    "workflowsDir": "./workflows",
    "adminDir": "./admin"
  }
}
```

## Ports & Server
- `atomo dev` starts a server (default port 3000). Override with `--port`.
- Standalone server: `atomo-server --config-dir services/<name> --port 3000`.
