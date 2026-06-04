// normalize-contact — a REAL, working Atomo JS plugin (Tier 1, via Javy/QuickJS).
//
// ABI (what the runtime actually does — not the aspirational @atomo-cc/plugin-sdk):
//   stdin  : { "hook": "<name>", "record": { ...the row... } }
//   stdout : { "record": { ...possibly modified... }, "effects": [ ... ] }
//
// Effects are permission-gated by plugin.toml `permissions`:
//   { emit:  { model, event, data } }  needs WriteEvents  -> published to the event stream
//   { dbQuery: { model, limit } }      needs ReadDatabase -> constrained read
//   { http:  { method, url, body? } }  needs HttpRequests -> outbound request
//
// Build:  javy build index.js -o plugin.wasm   (no Node/npm needed at runtime)

function readStdin() {
  const parts = [];
  const buf = new Uint8Array(4096);
  let n;
  while ((n = Javy.IO.readSync(0, buf)) > 0) parts.push(buf.slice(0, n));
  const total = parts.reduce((a, p) => a + p.length, 0);
  const all = new Uint8Array(total);
  let o = 0;
  for (const p of parts) { all.set(p, o); o += p.length; }
  return new TextDecoder().decode(all);
}

function writeStdout(obj) {
  Javy.IO.writeSync(1, new TextEncoder().encode(JSON.stringify(obj)));
}

const { hook, record } = JSON.parse(readStdin());
const out = { record: record || {}, effects: [] };

if (hook === "before_create" || hook === "before_update") {
  // Normalize: canonical email + trimmed name.
  if (typeof out.record.email === "string") {
    out.record.email = out.record.email.trim().toLowerCase();
  }
  if (typeof out.record.name === "string") {
    out.record.name = out.record.name.trim();
  }
}

if (hook === "after_create") {
  // Typed emit: a Notification.Created event the rest of the system can react to.
  out.effects.push({
    emit: {
      model: "Notification",
      event: "Created",
      data: { kind: "contact_welcome", email: out.record.email },
    },
  });
}

writeStdout(out);
