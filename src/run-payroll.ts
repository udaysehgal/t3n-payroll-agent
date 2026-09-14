/**
 * The payroll agent's pipeline run: validate-credentials -> compute-payroll
 * -> execute-disbursement -> finalize-audit, then submit-escalations for any
 * flagged line, then reads the audit trail back via list-audit-cycles /
 * get-audit-entry to prove no raw PII appears in it.
 *
 * Two invocation paths are exercised:
 *   - PRIMARY: the authenticated operator session (org owner) calling the
 *     contract directly via `T3nClient.executeAndDecode`. This is fully
 *     working end to end (see docs/sample-run.txt).
 *   - AGENT (stateless, `invoke()` with the agent's own `t3n_key_...` API
 *     key, no session): this is the intended "run it post-challenge without
 *     babysitting a session" path and is what setup.ts provisions the agent
 *     identity + delegation for. It currently fails with HTTP 403 even
 *     though both `member_delegation` (satisfied) and the org-level grant
 *     (written and readable via `OrgDataClient.getDelegation`) are in place —
 *     `discoverCheckDelegation` reports `org_delegation` as "missing" despite
 *     the grant existing. Logged as a confirmed platform bug in
 *     docs/BUGS.md. The code path below is left in and clearly labeled so
 *     the moment that's fixed upstream, switching is a one-line change
 *     (AGENT_INVOKE_WORKS = true).
 */
import "dotenv/config";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { invoke } from "@terminal3/t3n-sdk";
import { connect } from "./connect.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const STATE_PATH = path.join(__dirname, "..", ".setup-state.json");
const CONTRACT_VERSION = "0.1.1";

// Flip to true once the org_delegation bug (docs/BUGS.md) is fixed upstream —
// no other code change needed.
const AGENT_INVOKE_WORKS = false;

type State = {
  orgDid: string;
  agentDid: string;
  agentApiKey: string;
  contractId: string;
  baseUrl: string;
  myEmployeeId: string;
  myDid: string;
};

function loadState(): State {
  if (!fs.existsSync(STATE_PATH)) {
    throw new Error("No .setup-state.json found — run `npm run setup` first.");
  }
  return JSON.parse(fs.readFileSync(STATE_PATH, "utf8"));
}

function section(title: string) {
  console.log(`\n=== ${title} ===`);
}

async function main() {
  const state = loadState();
  console.log(`Agent:    ${state.agentDid}`);
  console.log(`Org:      ${state.orgDid}`);
  console.log(`Contract: ${state.contractId}@${CONTRACT_VERSION}`);
  console.log(`Invocation path: ${AGENT_INVOKE_WORKS ? "agent stateless invoke()" : "operator session executeAndDecode() [agent invoke() blocked, see docs/BUGS.md]"}`);

  const cycleId = `cycle-${new Date().toISOString().slice(0, 10)}-${Date.now().toString(36)}`;
  console.log(`Cycle: ${cycleId}`);

  let call: <T = unknown>(functionName: string, input: unknown, piiDid?: string) => Promise<T>;

  if (AGENT_INVOKE_WORKS) {
    call = (functionName, input, piiDid) =>
      invoke({
        baseUrl: state.baseUrl,
        apiKey: state.agentApiKey,
        request: {
          contract_id: state.contractId,
          contract_version: CONTRACT_VERSION,
          function_name: functionName,
          input,
          ...(piiDid ? { pii_did: piiDid } : {}),
        },
      });
  } else {
    const { t3n } = await connect();
    call = (functionName, input, piiDid) =>
      t3n.executeAndDecode({
        contract_id: state.contractId,
        contract_version: CONTRACT_VERSION,
        function_name: functionName,
        input,
        ...(piiDid ? { pii_did: piiDid } : {}),
      });
  }

  section("1. validate-credentials");
  console.log(JSON.stringify(await call("validate-credentials", { cycle_id: cycleId }), null, 2));

  section("2. compute-payroll");
  const computed = await call("compute-payroll", {
    org_id: state.orgDid,
    cycle_id: cycleId,
    pay_period_start: "2026-09-01",
    pay_period_end: "2026-09-15",
    batch_cap_cents: "10000000", // $100,000.00 cap for this run
    historical_baselines: {
      [state.myEmployeeId]: "540000", // last cycle's net pay, close to this cycle's -> should NOT flag
      "emp-002": "475000",
      // emp-003 has no baseline on file -> flagged purely on the absolute threshold
    },
    individual_disbursement_threshold_cents: "1500000", // SGD 15,000 (SDK default)
  });
  console.log(JSON.stringify(computed, null, 2));

  section("3. execute-disbursement");
  // The profile-linked employee's line needs pii_did == that employee's own
  // DID so the host resolves {{profile.*}} from THEIR profile. This demo has
  // one real (non-synthetic) T3N identity -- our own operator DID.
  const disbursed = await call("execute-disbursement", { cycle_id: cycleId }, state.myDid);
  console.log(JSON.stringify(disbursed, null, 2));

  section("4. submit-escalations");
  console.log(JSON.stringify(await call("submit-escalations", { cycle_id: cycleId }), null, 2));

  section("5. finalize-audit");
  const audit = await call("finalize-audit", { cycle_id: cycleId });
  console.log(JSON.stringify(audit, null, 2));

  section("6. get-audit-entry (read-back)");
  const auditEntry = await call("get-audit-entry", { cycle_id: cycleId });
  console.log(JSON.stringify(auditEntry, null, 2));

  section("7. list-audit-cycles (read-back)");
  console.log(JSON.stringify(await call("list-audit-cycles", { limit: 5 }), null, 2));

  section("PII check");
  const auditText = JSON.stringify(auditEntry);
  const rawBankFields = ["123-45-6789", "110000999", "000123456789", "000987654321", "110000000", "110000112"];
  const leaked = rawBankFields.filter((v) => auditText.includes(v));
  if (leaked.length > 0) {
    console.log(`FAIL: found raw bank values in the audit trail: ${leaked.join(", ")}`);
    process.exitCode = 1;
  } else {
    console.log("OK: no raw bank account/routing numbers appear anywhere in the audit entry.");
  }
}

main().catch((err) => {
  console.error("\nrun-payroll failed:", err?.message ?? err);
  process.exit(1);
});
