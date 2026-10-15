import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, readFileSync, existsSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync, execSync } from 'node:child_process'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const version = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version

function isolated(run) {
  const dir = mkdtempSync(join(tmpdir(), 'atomo-scaffold-'))
  try { run(dir) } finally { rmSync(dir, { recursive: true, force: true }) }
}

function cli(dir, args) {
  return spawnSync(process.execPath, [join(root, 'index.mjs'), ...args], {
    cwd: dir, encoding: 'utf8', timeout: 30_000,
  })
}

for (const template of ['crm', 'blog', 'ecommerce', 'default', 'fullstack-atomo']) {
  test(`real CLI scaffolds ${template} with packaged admin`, () => isolated(dir => {
    const result = cli(dir, ['example', '--template', template])
    assert.equal(result.status, 0, result.stderr)
    const target = join(dir, 'example')
    const canonical = template === 'fullstack-atomo' ? 'crm' : template
    assert.equal(readFileSync(join(target, 'atomo/schema.ts'), 'utf8'),
      readFileSync(join(root, 'templates', canonical, 'schema.ts'), 'utf8'))
    const pkg = JSON.parse(readFileSync(join(target, 'package.json'), 'utf8'))
    assert.equal(pkg.name, 'example')
    assert.equal(pkg.dependencies['@atomo-cc/client-sdk'], `^${version}`)
    const compose = readFileSync(join(target, 'docker-compose.yml'), 'utf8')
    assert.match(compose, /ghcr\.io\/atomo-cc\/atomo-server:latest/)
    assert.match(compose, /\.\/atomo\/schema\.ts:/)
    assert.doesNotMatch(compose, /Dockerfile|atomo-rs/)
    assert.match(readFileSync(join(target, 'README.md'), 'utf8'), /localhost:3000\/admin/)
  }))
}

test('invalid template and missing argument do not create output', () => isolated(dir => {
  for (const args of [['example', '--template', 'missing'], ['example', '--template'], []]) {
    const result = cli(dir, args)
    assert.equal(result.status, 1, result.stdout)
    assert.equal(existsSync(join(dir, 'example')), false)
  }
}))

test('existing output is never overwritten', () => isolated(dir => {
  assert.equal(cli(dir, ['example']).status, 0)
  const before = readFileSync(join(dir, 'example/package.json'), 'utf8')
  assert.equal(cli(dir, ['example', '--template', 'blog']).status, 1)
  assert.equal(readFileSync(join(dir, 'example/package.json'), 'utf8'), before)
}))

test('help advertises compatibility alias without creating output', () => isolated(dir => {
  const result = cli(dir, ['--help'])
  assert.equal(result.status, 0)
  assert.match(result.stdout, /fullstack-atomo -> crm/)
  assert.equal(existsSync(join(dir, 'example')), false)
}))

test('npm pack contains entry and canonical schemas, not workspace artifacts', () => {
  const [pack] = JSON.parse(execSync('npm pack --dry-run --json --ignore-scripts', {
    cwd: root, encoding: 'utf8', timeout: 60_000,
  }))
  const files = pack.files.map(file => file.path)
  assert.ok(files.includes('index.mjs'))
  for (const name of ['crm', 'blog', 'ecommerce', 'default']) {
    assert.ok(files.includes(`templates/${name}/schema.ts`))
  }
  assert.ok(files.every(file => !/node_modules|test-results|^tests\//.test(file)))
})
