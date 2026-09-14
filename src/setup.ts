/**
 * One-time provisioning for the payroll agent demo:
 *   1. create an organisation + a org-owned payroll agent identity
 *   2. register the compiled z-payroll WASM contract under the org's tenant
 *   3. create KV maps (employees / secrets / cycles / audit_cycles) and seed them
 *   4. set our own T3N profile's demo bank fields (for the PII-safe placeholder path)
 *   5. grant the agent org-level + member-level delegation to run the contract
 *
 * Idempotent-ish: safe to re-run for map/entry writes; org + agent creation
 * are NOT idempotent (each run mints a new org + a new agent identity), so
 * this only needs to run once. Re-running after the first successful run
 * will fail loudly on org creation — that's intentional (see README).
 */
import "dotenv/config";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import {
  TenantClient,
  createOrgDataClientFromSession,
  Z_PAYROLL_RUN_FUNCTIONS,
  Z_PAYROLL_AUDIT_READ_FUNCTIONS,
} from "@terminal3/t3n-sdk";
import { connect } from "./connect.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WASM_PATH = path.join(__dirname, "..", "contract", "z-payroll", "target", "wasm32-wasip2", "release", "z_payroll.wasm");
const CONTRACT_TAIL = "payroll";
const CONTRACT_VERSION = "0.1.1";
const BANK_HOST = "httpbin.org";
const ALL_FUNCTIONS = [...Z_PAYROLL_RUN_FUNCTIONS, ...Z_PAYROLL_AUDIT_READ_FUNCTIONS];

const STATE_PATH = path.join(__dirname, "..", ".setup-state.json");

function didHex(did: string): string {
  return did.replace(/^did:t3n:/, "");
}

async function main() {
  if (!fs.existsSync(WASM_PATH)) {
    throw new Error(`WASM not built yet. Run: (cd contract/z-payroll && cargo build --target wasm32-wasip2 --release)\nExpected: ${WASM_PATH}`);
  }

  const { t3n, did, baseUrl } = await connect();
  console.log(`Operator DID: ${did}`);

  console.log("\n[1/6] Creating organisation...");
  const orgDid = (await t3n.createOrganisation("T3N Payroll Agent Demo")).toString();
  console.log(`  org DID: ${orgDid}`);

  console.log("\n[2/6] Provisioning payroll agent identity...");
  const agentResult = await t3n.createAgent(orgDid, "payroll-agent", { defaultCard: false });
  const agentDid = agentResult.agentDid.toString();
  console.log(`  agent DID: ${agentDid}`);
  console.log(`  agent API key: ${agentResult.apiKey.slice(0, 14)}... (full key saved to .setup-state.json, gitignored, not printed)`);

  console.log("\n[3/6] Registering z-payroll contract...");
  const tenant = new TenantClient({ t3n, baseUrl });
  const wasm = fs.readFileSync(WASM_PATH);
  const registerResult = await tenant.contracts.register(
    { tail: CONTRACT_TAIL, version: CONTRACT_VERSION, wasm },
    { tenantTarget: orgDid },
  );
  console.log(`  registered: ${registerResult.name} (contract_id=${registerResult.contract_id})`);
  const contractId = registerResult.name;

  console.log("\n[4/6] Creating KV maps + seeding employee roster...");
  for (const tail of ["employees", "secrets", "cycles", "audit_cycles"]) {
    await tenant.maps.create(
      { tail, visibility: "private", writers: "all", readers: "all" },
      { tenantTarget: orgDid },
    );
    console.log(`  map created: ${tail}`);
  }

  await tenant.maps.entrySet("secrets", "bank_api_key", "mock-bank-demo-key-not-real", { tenantTarget: orgDid });

  const myEmployeeId = "emp-operator";
  const employees = [
    {
      employee_id: myEmployeeId,
      base_salary_cents: 550000, // $5,500.00 — the one REAL, profile-linked employee
      pii_source: "profile",
      employee_did_hex: didHex(did),
    },
    {
      employee_id: "emp-002",
      base_salary_cents: 480000, // $4,800.00 — synthetic demo employee
      pii_source: "synthetic",
      bank_account_number: "000123456789",
      bank_routing_number: "110000000",
    },
    {
      employee_id: "emp-003",
      base_salary_cents: 2_200_000, // $22,000.00 — deliberately over threshold, should get flagged/escalated
      pii_source: "synthetic",
      bank_account_number: "000987654321",
      bank_routing_number: "110000112",
    },
  ];
  for (const emp of employees) {
    await tenant.maps.entrySet("employees", emp.employee_id, JSON.stringify(emp), { tenantTarget: orgDid });
    console.log(`  employee seeded: ${emp.employee_id} (${emp.pii_source})`);
  }

  console.log("\n[5/6] Setting demo bank profile fields on operator DID (PII-safe placeholder source)...");
  // Terminal3's user-profile schema only accepts a closed Level-1 field set —
  // a custom `bank_account_number` key is rejected ("UnrecognizedKeys", see
  // docs/BUGS.md). We repurpose `ssn` / `address` to carry the demo bank
  // account/routing numbers; the contract resolves them via the same
  // host-side {{profile.<field>}} substitution a real field would use.
  await t3n.submitUserInput({
    profile: {
      first_name: "Demo",
      last_name: "Operator",
      ssn: "123-45-6789", // SSN-shaped per platform validation (3-2-4 digits) — stand-in for bank_account_number, not a real SSN
      address: "110000999", // stand-in for bank_routing_number, see note above
    },
  });
  console.log("  profile updated (ssn/address repurposed as demo bank account/routing numbers)");

  console.log("\n[6/6] Granting agent delegation (org-level + member-level)...");
  const orgData = createOrgDataClientFromSession(t3n, baseUrl);
  await orgData.addDelegationGrants({
    orgDid,
    grants: [
      {
        grantee: agentDid,
        contract_id: contractId,
        functions: ALL_FUNCTIONS,
        scopes: ["payroll"],
        // allowed_hosts is NOT accepted on an org-level grant (it's an
        // "agent edge only" field on the member-delegation grant below) —
        // org-level egress is set separately via setAgentEgress.
      },
    ],
  });
  console.log("  org-level delegation granted to agent");

  try {
    await orgData.setAgentEgress({
      orgDid,
      agentDid,
      contractId,
      allowedHosts: [BANK_HOST],
    });
    console.log("  agent egress allowlist set (bank host)");
  } catch (e: any) {
    console.log(`  WARNING: setAgentEgress failed (${e.message}) — continuing without it; synthetic-employee plain-http disbursement may fail egress checks. See docs/BUGS.md.`);
  }

  await t3n.updateMemberDelegation({
    grantee: agentDid,
    contract_id: contractId,
    functions: ALL_FUNCTIONS,
    scopes: ["payroll"],
    allowed_hosts: [BANK_HOST],
  });
  console.log("  member-level (own-profile) delegation granted to agent");

  // Egress allowlisting (`allowed_hosts`) is per (grantee, contract) edge.
  // The grant above covers the AGENT's calls; a direct session call made as
  // the operator (the path this demo actually uses — see docs/BUGS.md for
  // why the agent's own stateless invoke() path is currently blocked) needs
  // its own edge with grantee = the operator's own DID.
  await t3n.updateMemberDelegation({
    grantee: did,
    contract_id: contractId,
    functions: ALL_FUNCTIONS,
    scopes: ["payroll"],
    allowed_hosts: [BANK_HOST],
  });
  console.log("  member-level (self) delegation granted to operator DID (for direct-session execution)");

  const state = {
    orgDid,
    agentDid,
    agentApiKey: agentResult.apiKey,
    contractId,
    baseUrl,
    myEmployeeId,
    myDid: did,
  };
  fs.writeFileSync(STATE_PATH, JSON.stringify(state, null, 2));
  console.log(`\nSetup complete. State written to ${path.relative(process.cwd(), STATE_PATH)} (gitignored — contains the agent's live API key).`);
  console.log(`Run: npm run payroll`);
}

main().catch((err) => {
  console.error("\nSetup failed:", err?.message ?? err);
  process.exit(1);
});
