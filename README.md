# z-payroll — a PII-safe payroll agent on Terminal3 ADK

Built for the [Terminal3 ADK Superteam bounty](https://earn.superteam.fun) —
"build a useful, easy-to-maintain enterprise agent."

**What it does:** runs a company's pay cycle (validate → compute → disburse
→ escalate → audit) as an autonomous agent, and disburses money to an
employee's bank account **without the agent, its WASM contract, or its logs
ever holding that employee's raw account/routing number.** The number is
substituted into the outbound bank-transfer request by the Terminal3 host,
inside the TEE, between "the contract builds a JSON template with a
`{{profile.field}}` marker" and "the HTTPS request leaves the enclave." The
contract WASM — the thing an attacker would actually be able to inspect or
exfiltrate — never has the plaintext digits in memory.

This is [one of Terminal3's own listed reference use cases](https://terminal3.io/products/agent-developer-kit)
("Payroll Agent — reads HR data, triggers bank transfers without exposing
raw account numbers"), implemented as a real, running Rust/WASM contract
plus a small TypeScript orchestrator, tested live against the testnet
sandbox.

## Proof, not a claim

`docs/sample-run.txt` is real terminal output captured against the live
Terminal3 testnet sandbox on 2026-09-14 — not a mockup. The last
disbursement line in it:

```json
{
  "employee_id": "emp-operator",
  "amount_cents": 550000,
  "disbursed": true,
  "bank_ref": "ref-200",
  "pii_mode": "placeholder"
}
```

`"pii_mode": "placeholder"` means that line went through
`host:interfaces/http-with-placeholders`, not plain `http` — see "PII
model" below for what that actually guarantees. Grep `docs/sample-run.txt`
and the whole repo: the operator's demo bank account/routing digits
(`123-45-6789` / `110000999`, seeded in `setup.ts`) never appear anywhere
except in `setup.ts` itself, where they're written *into the user's own
T3N profile* — not into the contract, not into any log.

## Architecture

```
                 ┌─────────────────────────────────────────────┐
                 │  Terminal3 testnet node (TEE)                │
                 │                                               │
 operator/agent  │   z:<org>:payroll contract (this repo's WASM) │
 ───session/────►│   ┌───────────────────────────────────────┐  │
 invoke() call   │   │ validate-credentials                   │  │
                 │   │ compute-payroll        ──┐             │  │
                 │   │ execute-disbursement     │  KV maps:   │  │
                 │   │   ├─ profile employee ────┼─ employees │  │
                 │   │   │  → http-with-placeholders          │  │
                 │   │   │    {{profile.ssn}}/{{profile.address}}
                 │   │   │    resolved HERE, inside the host,  │  │
                 │   │   │    from the calling user's own      │  │
                 │   │   │    T3N profile — contract never     │  │
                 │   │   │    sees the plaintext                │  │
                 │   │   └─ synthetic employees                │  │
                 │   │      → plain http, KV-stored fake data  │  │
                 │   │ submit-escalations       ├─ cycles      │  │
                 │   │ finalize-audit           ├─ audit_cycles│  │
                 │   │ get-audit-entry          └─ secrets     │  │
                 │   │ list-audit-cycles                       │  │
                 │   └───────────────┬───────────────────────┘  │
                 └───────────────────┼───────────────────────────┘
                                      │ HTTPS (egress-allowlisted)
                                      ▼
                              mock bank (httpbin.org/post,
                              a stand-in — see "Mock bank" below)
```

## PII model

Two disbursement paths, deliberately not conflated:

- **Profile-linked employee** (`pii_source: "profile"`, exactly one in this
  demo — the operator's own T3N identity, since a real Terminal3 signup is
  needed per employee): the contract templates
  `"account_number": "{{profile.ssn}}"` into the outbound request body and
  calls `http-with-placeholders`. The host resolves the marker from *that
  employee's own* T3N profile — bound via `pii_did` on the call — and
  substitutes it after the contract hands the request to the host, before
  the HTTPS call fires. The contract's WASM, this repo's logs, and the
  finished audit entry never contain the digits. This is the real
  mechanism the "Payroll Agent" use case is about.
- **Synthetic employees** (`pii_source: "synthetic"`, the other two seeded
  by `setup.ts`): fake bank details sit directly in the `employees` KV map
  and get disbursed via plain `http`. There's no real human behind these —
  they exist to make the batch/threshold/escalation logic demoable with
  more than one line — and every response is tagged `"pii_mode":
  "synthetic_plain"` so a reader can never mistake this path for the
  PII-safe one.

**Why `ssn`/`address` instead of `bank_account_number`/`bank_routing_number`:**
Terminal3's user-profile schema currently only accepts a closed Level-1
field set and rejects custom keys — discovered while building this, see
`docs/BUGS.md` item 2. The substitution mechanism itself is identical either
way; only the field *name* is a workaround.

## Mock bank

There's no bank sandbox account wired up here — `bank_url()` in
`contract/z-payroll/src/payroll.rs` points at `https://httpbin.org/post`,
which echoes the request back as JSON over real HTTPS. This exercises the
genuine egress + placeholder-substitution path end to end (a real bank's
test API would need its own signup and secret provisioning, out of scope
for a bounty demo) — swap `bank_url()` and `bank_headers()` for a real
sandbox (e.g. a Stripe/Airwallex test endpoint) to go from "proves the
mechanism" to "actually moves test money."

## Pipeline

| Function | What it does |
|---|---|
| `validate-credentials` | Preconditions: employee roster present, bank secret configured, cycle not already finalized |
| `compute-payroll` | Net pay per employee; flags amounts over the per-employee threshold or >50% off the supplied historical baseline; enforces the run against a batch cap |
| `execute-disbursement` | Disburses every unflagged line (placeholder path for the profile-linked employee, plain `http` for synthetic ones) |
| `submit-escalations` | Collects flagged lines for human review |
| `finalize-audit` | Writes an immutable, PII-free audit entry |
| `get-audit-entry` / `list-audit-cycles` | Read-back for the audit trail |

Full request/response shapes: `contract/z-payroll/wit/world.wit`.

## Running it

Prerequisites: Node 20+, Rust + `rustup target add wasm32-wasip2`, and a
Terminal3 DID + API key — sign up via SSO at
[go.terminal3.io/adk-community](https://go.terminal3.io/adk-community) (free
sandbox test credits).

```bash
npm install
cp .env.example .env        # fill in your T3N_API_KEY

npm run connect             # sanity check: prints your DID

npm run build:contract      # compiles contract/z-payroll to wasm32-wasip2
npm run setup                # one-time: org, agent identity, contract
                             # registration, KV maps + demo employees,
                             # profile fields, delegation grants
npm run payroll              # runs one full pay cycle
```

`npm run setup` is **not** idempotent for org/agent/contract creation (each
run mints a new org + a new agent identity) — it's meant to run once. State
(org DID, agent DID + API key, contract id) is written to
`.setup-state.json` (gitignored — it holds a live API key) and read by
`npm run payroll`.

### A note on credits

Sandbox test credits are limited (20,000 on signup) and iterating on this
during development burned through them — `npm run setup` creates an org +
an agent identity + registers a WASM contract + creates 4 KV maps, each of
which costs credits; see `docs/BUGS.md` item 5 for the exact numbers we
hit. If `npm run setup` fails with `InsufficientCredit`, you're out — the
bounty's contact channel (see the listing) can top you up.

## What's genuinely working vs. what's a known gap

Everything in the pipeline table above runs live end to end — see
`docs/sample-run.txt` for real captured output of all seven functions,
including the PII-safety proof.

The one architectural piece that does **not** currently work is the
agent's own *stateless* `invoke()` call (its own `t3n_key_...` API key, no
session) — the intended "point a cron job at this, no session to babysit"
production shape. It fails with a platform-side authorization gap
(`org_delegation` reported missing despite the grant being written and
readable) fully repro'd in `docs/BUGS.md` item 1. `src/run-payroll.ts`
currently runs the pipeline through the authenticated operator's own
session instead (`AGENT_INVOKE_WORKS = false`), which is real and working
but means a human's session, not the agent's own credential, is what's
authorizing each run today. Flip that flag once the platform bug is fixed.

## Maintenance & handover

I'm planning to keep running and maintaining this myself post-challenge,
not hand it over. Concretely, that means:

- Fixing the two things blocking the production shape: the `invoke()` /
  `org_delegation` gap (`docs/BUGS.md` #1) and the `list-audit-cycles` scan
  (`docs/BUGS.md` #6), then flipping `AGENT_INVOKE_WORKS` on so the agent
  runs on its own credential instead of an operator session.
- Swapping the `httpbin.org` mock bank for a real test-mode payment
  provider (Stripe/Airwallex) once I wire one up, per the "Mock bank"
  section above.
- Making `npm run setup` idempotent (currently mints a fresh org/agent
  each run — fine for a one-off demo, not for a long-lived deployment).
- If Terminal3's startup program is a good fit for taking this further as
  a real product rather than a demo, I'd be interested in applying via the
  listing page.

No handover process needed for this submission — the repo stays under my
account, and the credentials in `.env`/`.setup-state.json` are mine and
gitignored (never committed).
