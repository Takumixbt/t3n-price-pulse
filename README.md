# price-pulse — a first T3N Agent Dev Kit contract

Agent ID, test credits, and a deployed Rust contract on Terminal3's T3N testnet.

## What I built

`price-pulse` is a minimal TEE contract: one exported function, `get-price`,
that calls CoinGecko's public price endpoint from inside the enclave and
returns a spot price stamped with T3N's own cluster timestamp. No secrets,
no PII, no KV writes — it imports only `tenant-context`, `logging`, and
`http`, which keeps it linked against the smallest capability world the ADK
offers (`tenant-http`). The idea was to test the thinnest possible path from
zero to a working, invokable contract before adding anything else.

Source, WIT interface, and the registration script are in this repo.

## Use case

Any agent that needs to reason about price before acting (a payments agent
deciding whether a quote is fair, a treasury agent rebalancing, a trading
agent sizing a position) needs a price feed it can trust came from a real
source and wasn't tampered with in transit. Running the fetch inside a TEE
contract means the agent gets back a value it can attest was actually
fetched from CoinGecko at `fetched_at_secs`, not injected by a compromised
host process — the same pattern generalizes to any external read (FX rates,
oracle feeds, inventory counts) an agent needs to trust without running its
own infrastructure.

## Identity + credits

Claimed a dev key and test credits from the [claim page](https://www.terminal3.io/claim-page),
authenticated, and confirmed tenant status:

```
Connected as: did:t3n:51cfebef5279596508dae8355cb2c86a3ae08efc
TenantClient ready: {
  tenant: 'did:t3n:51cfebef5279596508dae8355cb2c86a3ae08efc',
  label: 'testnet-dev',
  status: 'active',
  quotas: { max_contracts: 10, max_maps: 50, ... },
  created_at: 1786538158
}
```

Registered a public Agent ID (ERC-8004 card, hosted directly on the T3N
node — no external storage needed):

```
$ t3n agent host-card --file agent-card.json --env testnet
agent card hosted on T3N: https://cn-api.sg.testnet.t3n.terminal3.io/api/agent-card/did:t3n:51cfebef5279596508dae8355cb2c86a3ae08efc
```

Publicly resolvable — `curl` that URL and you get the card back verbatim.

## Deploying and invoking the contract

```
$ cargo build --target wasm32-wasip2 --release
$ cargo test          # 4 unit tests, native target
```

Registered and invoked from the same session:

```
registered z:51cfebef5279596508dae8355cb2c86a3ae08efc:price-pulse as contract id 630
self-grant authorized: get-price -> api.coingecko.com
get-price result: {
  coin_id: 'solana',
  vs_currency: 'usd',
  price: 76.21,
  fetched_at_secs: 1786542308
}
```

That's a real outbound HTTP call executed inside the TEE, returning a live
price. `wasm-tools component wit` on the compiled artifact confirms the
capability surface matches the source exactly — `tenant-context`, `logging`,
and `http` only. The `kv-store` import I left in `world.wit` for parity with
the reference world never shows up in the compiled component's import table
at all, since nothing in the code calls it — capability declaration is
genuinely tied to what you use, not what you import.

## Findings

**1. The reference `.cargo/config.toml` breaks `cargo test`.** The
`z-tenant-flight` example ships `[build] target = "wasm32-wasip2"`, which
becomes the default target for every cargo invocation in the project,
including `cargo test`. On Windows that fails immediately with `%1 is not a
valid Win32 application (os error 193)` — the test runner tries to execute
the compiled `.wasm` as a native binary. Fix: don't set a default target in
config.toml at all; pass `--target wasm32-wasip2` explicitly only on the
release build. `cargo test` then runs on the host target the way the "Test
your TEE contract" walkthrough page assumes.

**2. `docs.terminal3.io`'s quickstart is stale against the shipped SDK.**
Installed `@terminal3/t3n-sdk@4.36.0`. Two mismatches, both against the
Quickstart / Set Up Dev Env pages:

- `new T3nClient({ wasmComponent, handlers })` throws a `T3nConfigError`
  immediately — `trustAnchor` is now a required field (`{ expected_peer_ids,
  rtmr3_allowlist }` from `fetchTrustedManifest(env)`, or the explicit
  `{ unsafe_trust_server: true }` opt-out). The quickstart snippet has
  neither.
- `tenant.me()` doesn't exist on `TenantClient`. The real call is nested:
  `tenant.tenant.me()` (a `TenantNamespace` under `.tenant`). Found this by
  grepping the package's own (unobfuscated) `dist/index.d.ts` rather than
  the docs site.

**3. `tenant.tenant.claim()` (the testnet self-admit call) reliably 500s**
for a tenant that already went through the web claim page. Reproduced twice,
distinct request IDs both times:

```
RpcError: RPC Error: Internal error [51df47f8-fc34-4bdc-b048-640fd16390c3]
RpcError: RPC Error: Internal error [5df7b3b1-283d-4d55-853c-aeebff2e4aa2]
code: 'RPC_ERROR', rpcMethod: 'action.execute', httpStatus: -32603
```

Workaround: skip `.claim()` entirely and call `.tenant.me()` directly — it
correctly reports `status: 'active'` with credits already granted, so the
web-claim path and the SDK self-admit path appear to double-provision the
same thing, and the second one errors instead of hitting the documented
idempotent `already-admitted` response.

**4. CoinGecko 403s any request with no `User-Agent`.** Not a T3N bug, but
worth flagging since the `http` capability doesn't inject a default one —
first deploy failed with `contract error: coingecko HTTP 403: "Please add a
descriptive User-Agent..."`. Fixed by setting one explicitly in the request
headers; redeployed as contract version `0.1.1`.

**5. The published npm package is obfuscated, and its own `package.json`
repo link 404s.** `dist/index.esm.js` / `dist/index.js` (1.2MB each) are run
through what looks like `javascript-obfuscator` — hex-named identifiers, a
self-decoding string-array bootstrap, no source map. The package is
MIT-licensed and points to `github.com/Terminal-3/trinity` (directory
`client/t3n-sdk`) as its source, but that repository doesn't resolve at all
on GitHub. For an SDK that signs with a raw secp256k1 key, that's the one
thing I'd fix first — not because anything looked malicious (the dependency
graph is exactly what you'd expect: `@noble/curves`, `ethers`, Bytecode
Alliance's own `jco`/`preview2-shim`, and the type definitions expose real
TDX-attestation and manifest-signature verification, which is far more
engineering than a credential-harvesting package would bother with), but
because right now nobody outside the team can actually read the code that
handles a private key. Worth noting a third party independently hit a
different `T3nClient` constructor crash on an older SDK version
(`Init0ne/t3-sdk-bug-reproduction`, v1.0.0) — the constructor has clearly
been reshaped more than once without the docs catching up each time.

## Suggestions

- Update the Quickstart / Set Up Dev Env pages for the `trustAnchor`
  requirement and the `tenant.tenant.me()` nesting — both cost real time to
  work around and are trivial to fix in the docs.
- Either fix `.claim()`'s interaction with web-claimed tenants or update its
  docstring to say when it's safe to skip.
- Ship a source map (even a private/internal one you can use to symbolicate
  reported stack traces) or an unminified build channel for the npm package,
  and fix or remove the dead repository link.
