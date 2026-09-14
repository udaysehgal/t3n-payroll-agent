# Bugs / friction found while building this

All of these were hit against the live `testnet` sandbox while building the
payroll agent in this repo, on 2026-09-14, using `@terminal3/t3n-sdk@5.2.0`.
Exact error text is quoted verbatim (request ids included where the SDK
surfaced one) rather than paraphrased.

## 1. Org-level delegation grants are written successfully but not honored by `check-delegation` / agent `invoke()` (highest severity)

**Expected:** after `OrgDataClient.setDelegation` (or `addDelegationGrants`)
grants an org-owned agent `functions` on a `z:<tid>:<tail>` contract, that
agent's stateless `invoke()` call (`X-T3N-Api-Key` auth, no session) against
that contract should succeed.

**Actual:** the agent's `invoke()` call fails with a bare `HTTP 403` (the SDK
deliberately gives no body detail here — see `InvokeError`'s doc comment).
Diagnosing with `discoverCheckDelegation` shows why:

```json
{
  "authorised": false,
  "disclosed": true,
  "satisfied": [
    { "grant": "member_delegation", "contract": "z:<org-hex>:payroll", "functions": ["validate-credentials"], "scopes": [] }
  ],
  "missing": [
    { "grant": "org_delegation", "contract": "z:<org-hex>:payroll", "functions": ["validate-credentials"], "scopes": [] }
  ]
}
```

`org_delegation` is reported as **missing** even though `OrgDataClient.getDelegation({orgDid, contractId})` — called immediately after the write, no delay — echoes the grant back exactly as written:

```json
{
  "contract_id": "z:<org-hex>:payroll",
  "grants": [{
    "grantee": "did:t3n:<agent-hex>",
    "contract_id": "z:<org-hex>:payroll",
    "functions": ["compute-payroll","execute-disbursement","finalize-audit","submit-escalations","validate-credentials","get-audit-entry","list-audit-cycles"],
    "scopes": ["payroll"]
  }]
}
```

**Repro steps:**
1. `createOrganisation` -> `createAgent(orgDid, name)` -> `contracts.register` a `z:` tenant contract.
2. `createOrgDataClientFromSession(t3n, baseUrl).setDelegation({orgDid, contractId, grants: [{grantee: agentDid, contract_id: contractId, functions: [...], scopes: ["anything"]}]})` — returns `{status: "updated", tx_hash: "tx:..."}`.
3. Immediately `orgData.getDelegation({orgDid, contractId})` — echoes the grant correctly.
4. `discoverCheckDelegation({baseUrl, apiKey: agentApiKey}, {contract: contractId, pii_did: <a member who separately granted member_delegation to this agent>, functions: [...], scopes: [...]})` — `org_delegation` still reports as `missing`.
5. Same result whether the org grant is written via `setDelegation` (full replace) or `addDelegationGrants` (additive).

Tried and ruled out as the cause: calling `addOrganisationMember(orgDid, myDid, {setAsOriginator: true})` first (no change); waiting between write and read (no change, tested immediately and a few minutes later).

**Workaround used in this repo:** the payroll pipeline runs through the
authenticated **operator session** (`T3nClient.executeAndDecode`, org owner
calling their own org's contract directly) instead of the agent's stateless
`invoke()`. This works reliably — see `docs/sample-run.txt`. The
`invoke()` code path is still present in `src/run-payroll.ts` behind an
`AGENT_INVOKE_WORKS` flag, ready to flip on once this is fixed, since it's
the architecturally-intended "run the agent post-challenge without a
session" path.

## 2. `submitUserInput`'s profile schema rejects custom fields despite the SDK's own doc comment describing it as open

The SDK's `UserInputProfile` interface doc comment reads: *"the shape is
intentionally open so callers can add provider-specific fields the contract
knows how to merge"* and its TS type carries an index signature
(`[key: string]: unknown`). In practice, submitting any field outside the
canonical Level-1 set is rejected server-side:

```
RPC Error: Profile validation failed: ValidationResult { issues: [ValidationIssue { path: [], error: UnrecognizedKeys { keys: ["bank_account_number", "bank_routing_number"] } }] }
```

**Impact:** a contract that wants to resolve `{{profile.<custom-field>}}`
via `http-with-placeholders` for a field the platform hasn't special-cased
(anything other than `first_name`/`last_name`/`country_of_residence`/
`document_issuance_country`/`ssn`/`address`/`email_address`/`phone_number`/
`campaign_code`/`role`) has no way to get that value into the user's
profile in the first place — despite the `http-with-placeholders` WIT
interface itself imposing no such restriction (any `profile.*` path is
accepted at the placeholder-resolution layer).

**Workaround used in this repo:** the payroll disbursement demo repurposes
the existing `ssn` field to carry the demo bank account number and
`address` to carry the demo routing number (see `contract/z-payroll/src/payroll.rs`,
`disburse_via_placeholder`). This still genuinely exercises the real
mechanism (host-side substitution, contract never sees the plaintext) — it
just can't use a field named what it actually is.

**Secondary bug on the same call:** the `ssn` field itself has undocumented
format validation (`SSN must be 9 digits, optionally grouped as 3-2-4`) that
additionally rejects at least some syntactically-9-digit values (e.g.
`000555111` was rejected with the same message before a differently-shaped
value `123-45-6789` was accepted) — this validation isn't mentioned anywhere
in the `UserInputProfile` type's doc comments, so we discovered the exact
accepted shape by trial and error.

## 3. `createOrganisation` / `createAgent` return a `Did` object, but a raw string is needed almost everywhere else

`T3nClient.createOrganisation(name): Promise<Did>` and
`createAgent(...).agentDid: Did` return `{ value: string, toString(): string }`,
not a plain string. Passing the object directly into a later call that
expects a `string` (e.g. `createAgent(organisationDid: string, ...)`,
`OrgDataClient` inputs) is not caught by the SDK at runtime — with `tsx`
(no ahead-of-time type check) it silently serializes as
`{"value":"...","toString":{}}` on the wire and fails deep inside RPC
validation with a confusing, seemingly-unrelated error:

```
RPC Error: parse input: invalid type: map, expected a string at line 1 column 20
```

This is technically "working as typed" (TypeScript would catch it in a
strict build), but the failure mode when it's missed is unusually opaque —
nothing in the error mentions DIDs, organisations, or agents. A runtime
`typeof` guard (or accepting `string | Did` and calling `.toString()`
internally) in `createAgent`/other DID-consuming calls would turn this into
an immediate, legible error.

## 4. `OrgDataClient.setAgentEgress` reports a freshly-created org-owned agent as not owned by its org

Immediately after `createAgent(orgDid, name)` succeeds and the returned
`agentDid` is confirmed to work for `addDelegationGrants`/`setDelegation`
(item 1 above shows those calls accept it as a valid grantee), calling:

```ts
orgData.setAgentEgress({ orgDid, agentDid, contractId, allowedHosts: ["httpbin.org"] })
```

fails with:

```
RPC Error: NotAnOrgOwnedAgent: agent_did is not an agent owned by org_did
```

Not retried with a delay (this repo's `setup.ts` treats it as non-fatal and
continues — see the `try/catch` around the call), so we can't rule out a
short propagation lag, but it's inconsistent with `whoami` (via the agent's
own API key) reporting `organisations: [orgDid]` correctly in the same
setup run.

## 5. Sandbox credit accounting: map creation appears to cost 10,000 credits each

`TenantClient.maps.create` failed partway through seeding four maps with:

```
InsufficientCredit (account=<hex>, required=10000000000, available=0)
```

`10000000000` base units = 10,000 credits (balances are in base units per
`token.get-balance`; `available: 20000000000` at signup matches the
documented "20,000 test credits"). If each `maps.create` call genuinely
costs 10,000 credits — half of an entire signup grant — that's a very
different cost model than the product page's framing ("supporting ~25
agents and 5,000 protected actions"), and would mean a single tenant can
create at most ~2 maps on a fresh grant. This may instead be a reserved
*hold* rather than the actual settled cost (the error fired on our 4th
`maps.create` call in one run, after 3 identical calls had already
succeeded moments earlier in the same process, which doesn't fit "each call
costs 10k" cleanly either) — flagging as worth an accounting-clarity look
either way, since the error message doesn't distinguish "this call costs
10k" from "this call needs a 10k reservation held against your balance
regardless of settled cost."

## 6. `list-audit-cycles` (our own contract's `kv-store::scan`) returned an empty list right after `finalize-audit` wrote the entry `get-audit-entry` could read

In the captured run in `docs/sample-run.txt`, `get-audit-entry` for
`cycle-2026-09-14-mu0vmv49` (called right after `finalize-audit` wrote it)
correctly returns the full entry, but `list-audit-cycles` — called seconds
later in the same process, doing a `kv-store::scan("audit_cycles", [], [0xff;64], 1000)`
over the same map — returns `{"cycles": []}`. Not fully root-caused (could
be our own scan bounds, e.g. `end = [0xff; 64]` vs. how the host compares
key byte-lengths, rather than a host-side issue) — noted here rather than
silently reported as working, since we didn't verify the fix. If it
recurs after we widen/fix the scan bounds, it likely points at the host's
`kv-store::scan` rather than our own code.
