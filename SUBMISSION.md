# z-payroll — a PII-safe payroll agent on Terminal3 ADK

**Terminal3 ADK Superteam bounty submission**
Submitted by: udaysehgal18@gmail.com (GitHub: udaysehgal)
Repo: https://github.com/udaysehgal/t3n-payroll-agent

## What I built

A **Payroll Agent** — one of Terminal3's own listed reference use cases
("reads HR data, triggers bank transfers without exposing raw account
numbers"). It runs a company's pay cycle end to end as an autonomous
agent: validate → compute → disburse → escalate → audit, implemented as a
real Rust/WASM TEE contract plus a small TypeScript orchestrator.

The core claim it demonstrates concretely, not just in theory: **the
agent, its WASM contract, and its logs never hold an employee's raw bank
account/routing number.** The number is substituted into the outbound
bank-transfer HTTPS request by the Terminal3 host, inside the TEE, between
"the contract builds a JSON template with a `{{profile.field}}` marker"
and "the request leaves the enclave." I proved this rather than asserting
it — see below.

## Proof, not a claim

Everything below was captured from real runs against the live Terminal3
testnet sandbox on 2026-09-14 — see `docs/sample-run.txt` in the repo for
the full unedited terminal output of all 7 contract functions.

The disbursement line that matters:

```json
{
  "employee_id": "emp-operator",
  "amount_cents": 550000,
  "disbursed": true,
  "bank_ref": "ref-200",
  "pii_mode": "placeholder"
}
```

`"pii_mode": "placeholder"` marks the line that went through
`host:interfaces/http-with-placeholders` rather than plain `http`. I
grepped the whole repo and the sample-run log for the seeded demo bank
digits (`123-45-6789` / `110000999`) — they appear nowhere except in
`setup.ts`, where they're written into the user's own T3N profile, never
into the contract or the audit trail.

## Architecture

```
operator/agent          Terminal3 testnet node (TEE)
  session or    ──────►  z:<org>:payroll contract (this repo's WASM)
  invoke() call            validate-credentials
                           compute-payroll
                           execute-disbursement
                             ├─ profile employee → http-with-placeholders
                             │   {{profile.ssn}}/{{profile.address}} resolved
                             │   HOST-SIDE from the employee's own T3N
                             │   profile — contract never sees plaintext
                             └─ synthetic employees → plain http (demo data)
                           submit-escalations
                           finalize-audit  ──► KV-backed audit trail
                           get-audit-entry / list-audit-cycles
                                    │
                                    ▼  HTTPS (egress-allowlisted)
                            mock bank (httpbin.org/post — stand-in,
                            see README "Mock bank" for why)
```

## Usefulness & ease of maintenance (the judging focus)

- **Usefulness:** payroll disbursement is one of the highest-stakes,
  most PII-sensitive workflows in any enterprise. This shows Terminal3's
  actual differentiator — PII-safe agent execution with an audit trail —
  on that workflow, not a toy example.
- **Ease of maintenance:** small surface (one Rust crate, three TS
  scripts), no unnecessary abstraction, every function individually
  testable, and a README that documents exactly what's real vs. stubbed
  so a future maintainer isn't misled.
- **Running post-challenge:** see "Continuing to run this" below.

## What's genuinely working vs. known gaps

Everything in the pipeline (all 7 contract functions) runs live end to
end against the real sandbox — see `docs/sample-run.txt`.

One gap, documented rather than hidden: the agent's own *stateless*
`invoke()` path (its own API key, no session — the intended
"point a cron job at this" production shape) is blocked by what looks
like a platform bug: an org-level delegation grant is written and
read back correctly, but `check-delegation`/`invoke()` still reports it
missing. Full repro in `docs/BUGS.md` #1. Worked around for this demo by
running the pipeline through the operator's authenticated session
instead — genuinely working, just not the final intended shape yet.

## Bugs found (full detail in `docs/BUGS.md`)

1. **(Highest severity)** Org-level delegation grants are written and
   read back correctly via `OrgDataClient`, but not honored by
   `discoverCheckDelegation` / the agent's stateless `invoke()` — blocks
   the "agent runs on its own credential, no session" production path.
2. `UserInputProfile`'s doc comment describes the profile schema as
   open/extensible; in practice it's a closed field set and rejects
   custom keys server-side. Also: undocumented format validation on the
   `ssn` field (accepts `123-45-6789`, rejects at least one syntactically
   9-digit value tried).
3. `createOrganisation`/`createAgent` return `Did` objects, not plain
   strings; passing one into a call expecting a `string` fails at
   runtime with an opaque, unrelated-looking RPC error rather than a
   clear type error.
4. `setAgentEgress` fails with `NotAnOrgOwnedAgent` immediately after
   `createAgent`, even though the same agent DID is simultaneously
   accepted elsewhere (delegation grants, `whoami`) as belonging to that
   org.
5. Sandbox credit accounting: `maps.create` appears to require/cost
   10,000 of the 20,000 signup credits per call in some runs — worth an
   accounting-clarity look, exact numbers and caveats in `docs/BUGS.md`.
6. `list-audit-cycles` (our own contract's `kv-store::scan`) returned an
   empty list immediately after `finalize-audit` wrote an entry that
   `get-audit-entry` could read by exact key — not fully root-caused,
   flagged rather than silently claimed fixed.

## Continuing to run this

I'd like to keep running and maintaining this myself rather than hand it
over. Near-term plan:

- Fix the `invoke()`/`org_delegation` gap (bug #1) and the
  `list-audit-cycles` scan (bug #6), then flip the agent over to running
  on its own credential instead of an operator session.
- Swap the `httpbin.org` mock bank for a real test-mode payment provider
  (Stripe/Airwallex).
- Make `npm run setup` idempotent for longer-lived use instead of
  minting a fresh org/agent each run.
- If Terminal3's startup program is a good fit for taking this further,
  I'd like to apply via the listing page.

No handover process needed — repo stays under my GitHub account, and all
credentials used are mine (gitignored, never committed).

## Links

- Repo: https://github.com/udaysehgal/t3n-payroll-agent
- Sample run (proof): https://github.com/udaysehgal/t3n-payroll-agent/blob/main/docs/sample-run.txt
- Bugs found: https://github.com/udaysehgal/t3n-payroll-agent/blob/main/docs/BUGS.md
