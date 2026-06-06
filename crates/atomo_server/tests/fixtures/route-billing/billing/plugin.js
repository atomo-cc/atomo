// Billing route plugin (test fixture). Compiled with Javy v8.1.1:
//   javy build plugin.js -o plugin.wasm
// (self-contained, WASI-preview1; runs on the runtime's wasmtime 16.)
//
// Serves POST /ext/billing/debit: an atomic no-overdraw debit (transaction) plus an
// `emit` effect. Exercises the full phase-3 route path end to end.

function readInput() {
  const chunk = new Uint8Array(1024);
  let buf = new Uint8Array(0);
  let n;
  while ((n = Javy.IO.readSync(0, chunk)) > 0) {
    const next = new Uint8Array(buf.length + n);
    next.set(buf);
    next.set(chunk.subarray(0, n), buf.length);
    buf = next;
  }
  return new TextDecoder().decode(buf);
}

function writeOutput(s) {
  Javy.IO.writeSync(1, new TextEncoder().encode(s));
}

function handle(env) {
  if (env && env.route) {
    const body = env.route.body || {};
    return {
      transaction: [
        {
          sql: "UPDATE accounts SET balance = balance - $1 WHERE user_id = $2 AND balance >= $1",
          params: [body.cost, body.userId],
          expect: { minRowsAffected: 1, elseStatus: 402, elseBody: { result: "insufficient" } },
        },
      ],
      effects: [{ emit: { model: "Account", event: "Updated", data: { userId: body.userId } } }],
      response: { status: 200, body: { result: "applied", user: body.userId } },
    };
  }
  return (env && env.record) || {};
}

let env = {};
try {
  env = JSON.parse(readInput());
} catch (e) {}
writeOutput(JSON.stringify(handle(env)));
