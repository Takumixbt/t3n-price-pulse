# price-pulse — run book

Entry A for the LOL Ventures / Terminal3 "Create Agent ID, claim free tokens,
deploy first RUST contract" bounty. Capability surface: `http` only (public
CoinGecko price lookup) — no secrets, no PII, no kv-store writes.

## 0. Claim identity + credits (manual, browser)

1. Go to https://www.terminal3.io/claim-page and sign in with this entry's
   work email.
2. Copy the developer key immediately — it is shown exactly once.
3. Set it locally (do not paste it into chat / commit it):
   ```
   export T3N_API_KEY="0x<key>"
   ```

## 1. Build the contract (no credentials needed)

```
cargo build --target wasm32-wasip2 --release
cargo test
wasm-tools component wit target/wasm32-wasip2/release/price_pulse.wasm
```

## 2. Register a public Agent ID (CLI, needs T3N_API_KEY)

```
npx @terminal3/t3n-sdk --help
t3n whoami --env testnet
export AGENT_DID=$(t3n whoami --env testnet)
t3n agent create-card --did "$AGENT_DID" \
  --name "price-pulse agent" \
  --description "Reads a public spot price via a T3N TEE contract" \
  --force
t3n agent host-card --file agent-card.json --env testnet
curl https://<node>/api/agent-card/"$AGENT_DID"
```

## 3. Register + invoke the contract (needs T3N_API_KEY)

```
cd scripts
npm install
npx tsx quickstart.ts
```

Expect, in order: `Connected as: did:t3n:...`, `TenantClient ready.`,
`registered z:<tid>:price-pulse as contract id <n>`, the self-grant
confirmation, then the `get-price` JSON result.

Ran into a few SDK/docs mismatches along the way (missing `trustAnchor`,
`tenant.me()` vs `tenant.tenant.me()`, a `.claim()` 500) and one CoinGecko
403 from a missing `User-Agent`. Full writeup with real output, request IDs,
and fixes: see [README.md](README.md).
