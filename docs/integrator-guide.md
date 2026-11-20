# Integrator Guide: Building Pool Transactions

This guide details how backend services and third-party integrators construct, hash, simulate, and submit transactions against the SorobanPool `group_buy` smart contract using the generated TypeScript bindings.

---

## 1. On-Chain vs. Off-Chain Architecture

SorobanPool separates rich product data from financial settlement:
- **Off-Chain Catalog:** Full product details, images, descriptions, supplier details, delivery hub contact info, and localized terms live in off-chain databases.
- **On-Chain Terms (`PoolTerms`):** Immutable snapshot of financial parameters that govern funds (supplier address, category symbol, pricing tiers, MOQ, member caps, lead time, and perishable flag).
- **Integrity Anchors:**
  - `offer_hash`: 32-byte SHA-256 digest of the canonical offer JSON. Guarantees that off-chain product specifications cannot be altered once a pool is created.
  - `unit_label_hash`: 32-byte SHA-256 digest of the normalized unit label string (e.g. `"50kg bag"`).
  - `hub_hash`: 32-byte SHA-256 digest of the canonical delivery hub address and contact payload.

---

## 2. Canonical JSON Hashing (`offer_hash` and `hub_hash`)

To ensure deterministic hashing across different platforms and runtimes, hashes must be computed using JSON Canonicalization Scheme (RFC 8785 / JCS):
1. Keys must be sorted lexicographically by UTF-16 code units.
2. No unnecessary whitespace (compact representation).
3. Deterministic number formatting.
4. Compute SHA-256 over the canonical UTF-8 bytes.

### Canonical JSON Hashing Helper (TypeScript)

```typescript
import { createHash } from "node:crypto";

/**
 * Serializes an object to deterministic canonical JSON (RFC 8785).
 */
export function canonicalJsonStringify(obj: any): string {
  if (obj === null || typeof obj !== "object") {
    return JSON.stringify(obj);
  }
  if (Array.isArray(obj)) {
    return "[" + obj.map(canonicalJsonStringify).join(",") + "]";
  }
  const sortedKeys = Object.keys(obj).sort();
  const pairs = sortedKeys.map(
    (k) => `${JSON.stringify(k)}:${canonicalJsonStringify(obj[k])}`
  );
  return "{" + pairs.join(",") + "}";
}

/**
 * Returns 32-byte Buffer SHA-256 of canonical JSON.
 */
export function computeCanonicalHash(payload: object): Buffer {
  const canonicalStr = canonicalJsonStringify(payload);
  return createHash("sha256").update(canonicalStr, "utf8").digest();
}

/**
 * Computes unit label hash.
 */
export function computeUnitLabelHash(label: string): Buffer {
  return createHash("sha256").update(label.trim().toLowerCase(), "utf8").digest();
}
```

---

## 3. Constructing `PoolTerms` from an Offer

When an organizer initiates a pool from a catalog offer, map the offer terms into `PoolTerms`:

```typescript
import type { PoolTerms, Tier } from "group_buy";

interface CatalogOffer {
  id: string;
  supplierAddress: string;
  category: string;
  unitLabel: string;
  tiers: Array<{ minUnits: number; unitPriceUsdc: number }>;
  moq: number;
  maxUnits: number;
  maxPerMember: number;
  leadTimeHours: number;
  perishable: boolean;
  description: string;
}

export function buildPoolTerms(offer: CatalogOffer): PoolTerms {
  // 1. Compute canonical offer hash over immutable product snapshot
  const offerSnapshot = {
    id: offer.id,
    supplier: offer.supplierAddress,
    category: offer.category,
    unitLabel: offer.unitLabel,
    moq: offer.moq,
    maxUnits: offer.maxUnits,
    leadTimeHours: offer.leadTimeHours,
    perishable: offer.perishable,
  };
  const offerHash = computeCanonicalHash(offerSnapshot);
  const unitLabelHash = computeUnitLabelHash(offer.unitLabel);

  // 2. Convert USDC decimal prices to i128 stroops (7 decimals: 1 USDC = 10,000,000)
  const USDC_DECIMALS = 10_000_000n;
  const tiers: Tier[] = offer.tiers.map((t) => ({
    min_units: t.minUnits,
    unit_price: BigInt(Math.round(t.unitPriceUsdc * 1e7)),
  }));

  return {
    supplier: offer.supplierAddress,
    offer_hash: offerHash,
    unit_label_hash: unitLabelHash,
    category: offer.category,
    tiers,
    moq: offer.moq,
    max_units: offer.maxUnits,
    max_per_member: offer.maxPerMember,
    lead_time_secs: BigInt(offer.leadTimeHours * 3600),
    perishable: offer.perishable,
  };
}
```

---

## 4. Complete Runnable Snippet (Testnet)

This snippet demonstrates initializing the `Client`, creating a pool, inspecting quote deposits, and committing units.

```typescript
import { Buffer } from "buffer";
import { Keypair } from "@stellar/stellar-sdk";
import { Client as GroupBuyClient } from "group_buy";
import {
  buildPoolTerms,
  computeCanonicalHash,
} from "./helpers";

// Testnet deployment addresses from deployments/testnet.json
export const TESTNET_CONFIG = {
  rpcUrl: "https://soroban-testnet.stellar.org",
  networkPassphrase: "Test SDF Network ; September 2015",
  contracts: {
    groupBuy: "CAVVOU7XCSBZYIX6ZWYLG45YIB3WIAU2UQN4FGUGTFMGDDHXRG34OUZF",
    config: "CCMCEWYED4Z5JNRV57VTSXA7FISK7EYST7RUP3LPAINXMO3QNGKGRDAN",
    usdc: "CB4Y5G47MT73ZVDAGBTSMSKAXRSHU6JYBHI7O3XAUQNKBCBVTEBZOCYB",
  },
};

async function main() {
  const client = new GroupBuyClient({
    contractId: TESTNET_CONFIG.contracts.groupBuy,
    networkPassphrase: TESTNET_CONFIG.networkPassphrase,
    rpcUrl: TESTNET_CONFIG.rpcUrl,
  });

  // Keypair for authorized organizer / member
  const organizerKeypair = Keypair.random(); // In production, use registered organizer key/passkey
  const memberKeypair = Keypair.random();

  console.log("Organizer:", organizerKeypair.publicKey());
  console.log("Member:", memberKeypair.publicKey());

  // 1. Build delivery hub payload and canonical hash
  const hubPayload = {
    hubId: "hub-lagos-island-01",
    name: "Lagos Island Distribution Hub",
    address: "12 Marina Street, Lagos Island",
    contactPhone: "+2348012345678",
  };
  const hubHash = computeCanonicalHash(hubPayload);

  // 2. Build PoolTerms from catalog offer
  const terms = buildPoolTerms({
    id: "offer-grains-2026-001",
    supplierAddress: "CDASLXELXFN4MFBRHHG2MC75KLI4EVU7Y5RQKMTJINGXF2PJOELZOTEA",
    category: "grains",
    unitLabel: "50kg bag",
    moq: 100,
    maxUnits: 500,
    maxPerMember: 50,
    leadTimeHours: 48,
    perishable: false,
    description: "Premium White Maize 50kg bags",
    tiers: [
      { minUnits: 100, unitPriceUsdc: 25.0 }, // 25.00 USDC
      { minUnits: 250, unitPriceUsdc: 23.5 }, // 23.50 USDC
      { minUnits: 400, unitPriceUsdc: 22.0 }, // 22.00 USDC
    ],
  });

  const nowSecs = BigInt(Math.floor(Date.now() / 1000));
  const fillDeadline = nowSecs + 86400n * 3n; // 3 days fill deadline

  console.log("Submitting create_pool transaction...");
  const createTx = await client.create_pool({
    organizer: organizerKeypair.publicKey(),
    terms,
    hub_hash: hubHash,
    organizer_fee_bp: 100, // 1.00% organizer fee
    cluster: "lagos",
    fill_deadline: fillDeadline,
  });

  // Sign and submit (requires registered organizer authorization)
  createTx.signAuthEntries({
    publicKey: organizerKeypair.publicKey(),
    signAuthEntry: async (entry) => organizerKeypair.sign(entry),
  });
  // const poolId = await createTx.signAndSend();
  // console.log("Created pool ID:", poolId);

  const demoPoolId = 1n;

  // 3. Quote commitment deposit
  console.log(`Getting quote for committing 20 units to pool ${demoPoolId}...`);
  const quoteTx = await client.quote_commit({
    pool_id: demoPoolId,
    units: 20,
  });
  const quoteResult = await quoteTx.simulate();
  console.log("Required escrow deposit (stroops):", quoteResult.result);

  // 4. Commit units
  console.log("Constructing commit transaction...");
  const commitTx = await client.commit({
    member: memberKeypair.publicKey(),
    pool_id: demoPoolId,
    units: 20,
  });
  console.log("Ready to sign and send commit transaction:", commitTx.built);
}

if (require.main === module) {
  main().catch(console.error);
}
```

---

## 5. Key Error Codes for Integrators

| Error Code | Error Name | Meaning & Resolution |
|---|---|---|
| `302` | `BadTerms` | MOQ does not equal tier[0].min_units, tiers are not strictly descending in price, or max_units < MOQ. |
| `303` | `CategoryNotAllowed` | Category symbol has not been enabled in `config.set_category`. |
| `304` | `NotVerified` | Organizer, supplier, or trader is not registered or is suspended in `registry`. |
| `305` | `BadDeadline` | Fill deadline exceeds maximum fill window or is in the past. |
| `314` | `ClusterMismatch` | Member's registered cluster does not match pool's cluster restriction. |
| `315` | `OverMemberLimit` | Units exceed pool's `max_per_member`. |
| `322` | `WithdrawLocked` | Member attempted to withdraw within the 2-hour pre-deadline lock window. |
| `323` | `WouldLowerTier` | Member withdrawal would drop total units below an already unlocked price tier (forbidden by ADR 0001). |
