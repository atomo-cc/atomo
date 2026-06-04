#!/usr/bin/env node
// Scaffold a new Atomo project — no Rust required.
//
// Usage:  npm create @atomo-cc/app <name> [-- --template crm|blog|ecommerce|default]
//   or:   npx @atomo-cc/create-app <name> --template crm
//
// Writes a starter project (schema + Docker compose + README) that runs with
// `docker compose up` against the published atomo-server image. This is the
// JS-only scaffolder — the Rust `atomo init` produces the same layout, but this
// one needs no toolchain. The heavy CLI commands (dev/generate) remain Rust.

import { existsSync } from 'node:fs'
import { mkdir, cp, writeFile, readdir } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const templatesDir = join(here, 'templates')

function fail(msg) {
  console.error(`\x1b[31merror\x1b[0m ${msg}`)
  process.exit(1)
}

async function availableTemplates() {
  try {
    return (await readdir(templatesDir, { withFileTypes: true }))
      .filter((d) => d.isDirectory())
      .map((d) => d.name)
      .sort()
  } catch {
    return []
  }
}

function parseArgs(argv) {
  let name
  let template = 'crm'
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a === '--template' || a === '-t') template = argv[++i]
    else if (a === '-h' || a === '--help') return { help: true }
    else if (!a.startsWith('-') && !name) name = a
  }
  return { name, template }
}

function projectPackageJson(name) {
  return JSON.stringify(
    {
      name,
      version: '0.1.0',
      description: 'An Atomo Content Core project',
      scripts: {
        dev: 'atomo dev',
        build: 'atomo build',
        'atomo:generate': 'atomo generate',
        'atomo:migrate': 'atomo migrate',
      },
      dependencies: {
        '@atomo-cc/client-sdk': '^0.1.0',
      },
      devDependencies: {
        typescript: '^5.0.0',
      },
    },
    null,
    2,
  )
}

const COMPOSE = `# Run this Atomo project with no Rust toolchain: \`docker compose up\`.
# Pulls the prebuilt atomo-server image and points it at ./atomo/schema.ts.
services:
  db:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: atomo
      POSTGRES_PASSWORD: atomo
      POSTGRES_DB: atomo_dev
    volumes:
      - atomo-db:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U atomo -d atomo_dev"]
      interval: 5s
      timeout: 3s
      retries: 12

  server:
    image: ghcr.io/chris533/atomo-server:latest
    depends_on:
      db:
        condition: service_healthy
    environment:
      DATABASE_URL: postgresql://atomo:atomo@db:5432/atomo_dev
      ATOMO_SCHEMA_PATH: /app/atomo/schema.ts
      # Dev-only secret/credentials — override for anything real.
      JWT_SECRET: dev-insecure-secret-change-me
      ADMIN_EMAIL: admin@example.com
      ADMIN_PASSWORD: change-me-too
      PORT: "3000"
    ports:
      - "3000:3000"
    volumes:
      - ./atomo/schema.ts:/app/atomo/schema.ts:ro

volumes:
  atomo-db:
`

function readme(name) {
  return `# ${name}

A new Atomo Content Core project.

## Run it (no Rust required)

The fastest way: Docker pulls the prebuilt server image and runs it against your
schema, alongside a Postgres database. No Rust, no toolchain to install.

\`\`\`bash
docker compose up                    # http://localhost:3000
curl http://localhost:3000/health    # -> OK
\`\`\`

The image bundles the **Admin UI** at <http://localhost:3000/admin>. Your data
model lives in \`atomo/schema.ts\`; edit it and re-run \`docker compose up\` to apply
changes — the server re-parses the schema and runs migrations on boot.

## Develop with the CLI (optional, needs Rust)

If you have the Atomo CLI installed, you also get schema hot-reload:

\`\`\`bash
npm install
npm run atomo:generate   # generate typed client SDK from the schema
npm run dev              # atomo dev — hot reload (requires the Rust toolchain)
\`\`\`

## Project Structure

- \`atomo/schema.ts\` - Your content model definitions
- \`docker-compose.yml\` - Run the server + Postgres with no Rust
- \`generated/\` - Auto-generated client code
- \`src/\` - Your application code

## Documentation

Visit [atomo.cc/docs](https://atomo.cc/docs) for full documentation.
`
}

const GITIGNORE = `node_modules/
dist/
generated/
.env
`

async function main() {
  const { name, template, help } = parseArgs(process.argv.slice(2))
  const templates = await availableTemplates()

  if (help) {
    console.log(
      `Usage: npm create @atomo-cc/app <name> [-- --template <name>]\n` +
        `Templates: ${templates.join(', ')}`,
    )
    return
  }
  if (!name) fail('missing project name. Usage: npm create @atomo-cc/app <name>')

  const schemaSrc = join(templatesDir, template, 'schema.ts')
  if (!existsSync(schemaSrc)) {
    fail(`unknown template "${template}". Available: ${templates.join(', ')}`)
  }

  const targetDir = resolve(process.cwd(), name)
  if (existsSync(targetDir)) fail(`directory "${name}" already exists`)

  // Scaffold.
  await mkdir(join(targetDir, 'atomo'), { recursive: true })
  await cp(schemaSrc, join(targetDir, 'atomo', 'schema.ts'))
  await writeFile(join(targetDir, 'package.json'), projectPackageJson(name))
  await writeFile(join(targetDir, 'docker-compose.yml'), COMPOSE)
  await writeFile(join(targetDir, 'README.md'), readme(name))
  await writeFile(join(targetDir, '.gitignore'), GITIGNORE)

  const g = '\x1b[32m'
  const c = '\x1b[36m'
  const r = '\x1b[0m'
  console.log(`\n${g}✓${r} Created ${c}${name}${r} (template: ${template})\n`)
  console.log('Next steps:')
  console.log(`  cd ${c}${name}${r}`)
  console.log(`  ${c}docker compose up${r}        # API on :3000, Admin UI on :3000/admin — no Rust`)
  console.log('')
}

main().catch((e) => fail(e?.message || String(e)))
