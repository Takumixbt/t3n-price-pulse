// price-pulse live registration script.
//
// Prereqs:
//   - T3N_API_KEY env var set to the key from https://www.terminal3.io/claim-page
//   - target/wasm32-wasip2/release/price_pulse.wasm already built
//     (cargo build --target wasm32-wasip2 --release, run from the entry root)
//
// Run from this scripts/ folder: npx tsx quickstart.ts

import {
  T3nClient,
  setEnvironment,
  loadWasmComponent,
  eth_get_address,
  metamask_sign,
  createEthAuthInput,
  TenantClient,
  getNodeUrl,
  getScriptVersion,
  fetchTrustedManifest,
} from "@terminal3/t3n-sdk";
import { readFile } from "fs/promises";

setEnvironment("testnet");

const T3N_API_KEY = process.env.T3N_API_KEY!;
if (!T3N_API_KEY) throw new Error("Set T3N_API_KEY before running (see claim page).");

const wasmComponent = await loadWasmComponent();
const address = eth_get_address(T3N_API_KEY);

// Required as of SDK v4.x (undocumented in the docs.terminal3.io quickstart,
// which still shows a pre-trustAnchor constructor call — see NOTES.md).
// This pins the node's DKG attestation instead of the unsafe bypass.
const trustAnchor = await fetchTrustedManifest("testnet");

const t3n = new T3nClient({
  wasmComponent,
  trustAnchor,
  handlers: { EthSign: metamask_sign(address, undefined, T3N_API_KEY) },
});

await t3n.handshake();
const did = await t3n.authenticate(createEthAuthInput(address));
const tenantDid = did.value; // did:t3n:... — never hardcode, always read back
console.log("Connected as:", tenantDid);

const tenant = new TenantClient({ t3n, baseUrl: getNodeUrl(), tenantDid });
// Real API is tenant.tenant.{claim,me}() — docs.terminal3.io's `tenant.me()`
// is stale for SDK v4.x. `.claim()` (testnet self-admit) is consistently
// throwing a server-side RPC_ERROR/Internal error for this already-claimed-
// via-web tenant (reproduced twice, distinct request_ids logged in
// NOTES.md) — skip it and confirm status via `.me()` alone instead.
const me = await tenant.tenant.me();
console.log("TenantClient ready:", me);

// --- Step: register the compiled contract ---
const WASM_PATH = "../target/wasm32-wasip2/release/price_pulse.wasm";
const CONTRACT_TAIL = "price-pulse";
const CONTRACT_VERSION = "0.1.1";

const wasmBytes = await readFile(WASM_PATH);
const registered = await tenant.contracts.register({
  tail: CONTRACT_TAIL,
  version: CONTRACT_VERSION,
  wasm: wasmBytes,
});
const contractId = registered.contract_id;
const tenantId = tenantDid.slice("did:t3n:".length);
const scriptName = `z:${tenantId}:${CONTRACT_TAIL}`;
console.log(`registered ${scriptName} as contract id ${contractId}`);

// --- Step: self-grant egress + invoke ---
// This is a direct/self call (no separate third-party agent): we act as our
// own user and our own agent, so the grant's agentDid is our own tenantDid.
// See "Outbound HTTP is authorized by the user, not the contract".
const scriptVersion = await getScriptVersion(getNodeUrl(), scriptName);
const userContractVersion = await getScriptVersion(getNodeUrl(), "tee:user/contracts");

await t3n.execute({
  script_name: "tee:user/contracts",
  script_version: userContractVersion,
  function_name: "agent-auth-update",
  input: {
    agents: [
      {
        agentDid: tenantDid,
        scripts: [
          {
            scriptName,
            versionReq: scriptVersion,
            functions: ["get-price"],
            allowedHosts: ["api.coingecko.com"],
          },
        ],
      },
    ],
  },
});
console.log("self-grant authorized: get-price -> api.coingecko.com");

const priceResp = await t3n.executeAndDecode({
  script_name: scriptName,
  script_version: scriptVersion,
  function_name: "get-price",
  input: { coin_id: "solana", vs_currency: "usd" },
});
console.log("get-price result:", priceResp);
