//! Core payroll pipeline logic, shared by every exported function.
//!
//! KV maps (tails; host auto-prefixes `z:<tid>:`):
//!   - `employees`    : employee_id -> Employee (JSON)
//!   - `cycles`       : cycle_id -> CycleRecord (JSON, working state)
//!   - `audit_cycles` : cycle_id -> AuditEntry (JSON, immutable once written)
//!   - `secrets`      : "bank_api_key" -> mock bank API token (bytes)
//!
//! PII model: an Employee with `pii_source: "profile"` has NO bank fields in
//! KV at all — `execute-disbursement` resolves them from the calling user's
//! own T3N profile via `http-with-placeholders`, scoped to that employee's
//! own DID as `pii_did` on the call. An Employee with `pii_source:
//! "synthetic"` carries fake bank fields directly in KV (there is no real
//! human behind it) and is disbursed via plain `http` — never presented as
//! PII-protected. See README.md.

use std::collections::BTreeMap;

#[derive(serde::Deserialize)]
struct CycleIdReq {
    cycle_id: String,
}

#[derive(serde::Serialize)]
struct ValidateResp {
    cycle_id: String,
    status: String,
    employee_count: u32,
}

#[derive(serde::Deserialize)]
struct ComputePayrollReq {
    org_id: String,
    cycle_id: String,
    pay_period_start: String,
    pay_period_end: String,
    /// Wire-encoded as a JSON string (see SDK `WireBlock.u64_as_string`).
    batch_cap_cents: String,
    historical_baselines: BTreeMap<String, String>,
    individual_disbursement_threshold_cents: Option<String>,
}

const DEFAULT_THRESHOLD_CENTS: u64 = 1_500_000; // SGD 15,000, mirrors SDK default

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct Employee {
    employee_id: String,
    base_salary_cents: u64,
    pii_source: String, // "profile" | "synthetic"
    #[serde(default)]
    employee_did_hex: Option<String>, // required when pii_source == "profile"
    #[serde(default)]
    bank_account_number: Option<String>, // synthetic only
    #[serde(default)]
    bank_routing_number: Option<String>, // synthetic only
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
struct PayrollLine {
    employee_id: String,
    amount_cents: u64,
    flagged: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    flag_reason: Option<String>,
    #[serde(default)]
    disbursed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    bank_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pii_mode: Option<String>, // "placeholder" | "synthetic_plain"
    #[serde(skip_serializing_if = "Option::is_none")]
    disbursement_error: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct Escalation {
    employee_id: String,
    reason: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct CycleRecord {
    cycle_id: String,
    status: String, // validated | computed | disbursed | finalized
    org_id: Option<String>,
    pay_period_start: Option<String>,
    pay_period_end: Option<String>,
    total_cents: u64,
    lines: Vec<PayrollLine>,
    escalations: Vec<Escalation>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct AuditEntry {
    cycle_id: String,
    finalized_at_secs: u64,
    total_cents: u64,
    employee_count: u32,
    disbursed_count: u32,
    flagged_count: u32,
    lines: Vec<PayrollLine>,
}

// ---------------------------------------------------------------------
// Entry points (native-target stubs; wasm32 impls below)
// ---------------------------------------------------------------------

pub fn validate_credentials(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: CycleIdReq =
        serde_json::from_slice(input).map_err(|e| format!("validate-credentials: bad input: {e}"))?;
    #[cfg(target_arch = "wasm32")]
    {
        wasm::validate_credentials(req)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("validate_credentials is only implemented on the wasm32 target".to_string())
    }
}

pub fn compute_payroll(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: ComputePayrollReq =
        serde_json::from_slice(input).map_err(|e| format!("compute-payroll: bad input: {e}"))?;
    #[cfg(target_arch = "wasm32")]
    {
        wasm::compute_payroll(req)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("compute_payroll is only implemented on the wasm32 target".to_string())
    }
}

pub fn execute_disbursement(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: CycleIdReq =
        serde_json::from_slice(input).map_err(|e| format!("execute-disbursement: bad input: {e}"))?;
    #[cfg(target_arch = "wasm32")]
    {
        wasm::execute_disbursement(req)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("execute_disbursement is only implemented on the wasm32 target".to_string())
    }
}

pub fn finalize_audit(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: CycleIdReq =
        serde_json::from_slice(input).map_err(|e| format!("finalize-audit: bad input: {e}"))?;
    #[cfg(target_arch = "wasm32")]
    {
        wasm::finalize_audit(req)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("finalize_audit is only implemented on the wasm32 target".to_string())
    }
}

pub fn submit_escalations(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: CycleIdReq =
        serde_json::from_slice(input).map_err(|e| format!("submit-escalations: bad input: {e}"))?;
    #[cfg(target_arch = "wasm32")]
    {
        wasm::submit_escalations(req)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("submit_escalations is only implemented on the wasm32 target".to_string())
    }
}

pub fn get_audit_entry(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: CycleIdReq =
        serde_json::from_slice(input).map_err(|e| format!("get-audit-entry: bad input: {e}"))?;
    #[cfg(target_arch = "wasm32")]
    {
        wasm::get_audit_entry(req)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("get_audit_entry is only implemented on the wasm32 target".to_string())
    }
}

#[derive(serde::Deserialize, Default)]
struct ListCyclesReq {
    limit: Option<u32>,
}

pub fn list_audit_cycles(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: ListCyclesReq = if input.is_empty() {
        ListCyclesReq::default()
    } else {
        serde_json::from_slice(input).map_err(|e| format!("list-audit-cycles: bad input: {e}"))?
    };
    #[cfg(target_arch = "wasm32")]
    {
        wasm::list_audit_cycles(req)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("list_audit_cycles is only implemented on the wasm32 target".to_string())
    }
}

// ---------------------------------------------------------------------
// wasm32 implementations
// ---------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use crate::host::{
        interfaces::{http as http_iface, http_with_placeholders as hwp, kv_store, logging},
        tenant::tenant_context,
    };

    fn tenant_map(tail: &str) -> String {
        let tid = tenant_context::tenant_did();
        format!("z:{}:{}", hex::encode(tid), tail)
    }

    fn kv_get_json<T: serde::de::DeserializeOwned>(map_tail: &str, key: &str) -> Result<Option<T>, String> {
        let map = tenant_map(map_tail);
        let bytes = kv_store::get(&map, key.as_bytes()).map_err(|e| format!("kv read {map}/{key}: {e}"))?;
        match bytes {
            None => Ok(None),
            Some(b) => serde_json::from_slice(&b)
                .map(Some)
                .map_err(|e| format!("kv decode {map}/{key}: {e}")),
        }
    }

    fn kv_put_json<T: serde::Serialize>(map_tail: &str, key: &str, value: &T) -> Result<(), String> {
        let map = tenant_map(map_tail);
        let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        kv_store::put(&map, key.as_bytes(), &bytes).map_err(|e| format!("kv write {map}/{key}: {e}"))
    }

    fn list_employees() -> Result<Vec<Employee>, String> {
        let map = tenant_map("employees");
        // Half-open scan across the whole key space (employee ids are short
        // ASCII strings, well below 0xFF-repeated upper bound in practice).
        let end = vec![0xffu8; 64];
        let rows = kv_store::scan(&map, &[], &end, 1000).map_err(|e| format!("kv scan {map}: {e}"))?;
        rows.into_iter()
            .map(|(_, v)| serde_json::from_slice::<Employee>(&v).map_err(|e| format!("employee decode: {e}")))
            .collect()
    }

    fn get_bank_api_key() -> Result<String, String> {
        let map = tenant_map("secrets");
        let bytes = kv_store::get(&map, b"bank_api_key")
            .map_err(|e| format!("kv read: {e}"))?
            .ok_or_else(|| "bank_api_key not found in z:<tid>:secrets — run setup.ts first".to_string())?;
        String::from_utf8(bytes).map_err(|e| e.to_string())
    }

    fn now_secs() -> u64 {
        tenant_context::cluster_timestamp_secs()
    }

    pub fn validate_credentials(req: CycleIdReq) -> Result<Vec<u8>, String> {
        let employees = list_employees()?;
        if employees.is_empty() {
            return Err("validate-credentials: no employees provisioned — run setup.ts".to_string());
        }
        let _ = get_bank_api_key()?; // fail fast if the operator forgot to seed it

        if let Some(existing) = kv_get_json::<CycleRecord>("cycles", &req.cycle_id)? {
            if existing.status == "finalized" {
                return Err(format!(
                    "validate-credentials: cycle {} is already finalized",
                    req.cycle_id
                ));
            }
        }

        let record = CycleRecord {
            cycle_id: req.cycle_id.clone(),
            status: "validated".to_string(),
            org_id: None,
            pay_period_start: None,
            pay_period_end: None,
            total_cents: 0,
            lines: Vec::new(),
            escalations: Vec::new(),
        };
        kv_put_json("cycles", &req.cycle_id, &record)?;

        let _ = logging::info(&format!(
            "validate-credentials: cycle {} ok, {} employees",
            req.cycle_id,
            employees.len()
        ));

        let resp = ValidateResp {
            cycle_id: req.cycle_id,
            status: "validated".to_string(),
            employee_count: employees.len() as u32,
        };
        serde_json::to_vec(&resp).map_err(|e| e.to_string())
    }

    pub fn compute_payroll(req: ComputePayrollReq) -> Result<Vec<u8>, String> {
        let cap_cents: u64 = req
            .batch_cap_cents
            .parse()
            .map_err(|_| "compute-payroll: batch_cap_cents is not a valid integer string".to_string())?;
        let threshold: u64 = match &req.individual_disbursement_threshold_cents {
            Some(s) => s
                .parse()
                .map_err(|_| "compute-payroll: individual_disbursement_threshold_cents is not a valid integer string".to_string())?,
            None => DEFAULT_THRESHOLD_CENTS,
        };

        let mut record: CycleRecord = kv_get_json("cycles", &req.cycle_id)?
            .ok_or_else(|| format!("compute-payroll: unknown cycle {} — call validate-credentials first", req.cycle_id))?;
        if record.status != "validated" {
            return Err(format!(
                "compute-payroll: cycle {} is in status '{}', expected 'validated'",
                req.cycle_id, record.status
            ));
        }

        let employees = list_employees()?;
        let mut lines = Vec::with_capacity(employees.len());
        let mut total: u64 = 0;

        for emp in &employees {
            let amount = emp.base_salary_cents;
            total = total.saturating_add(amount);

            let mut flagged = false;
            let mut reason = None;

            if amount > threshold {
                flagged = true;
                reason = Some(format!("amount {amount} exceeds per-employee threshold {threshold}"));
            } else if let Some(baseline_str) = req.historical_baselines.get(&emp.employee_id) {
                if let Ok(baseline) = baseline_str.parse::<u64>() {
                    if baseline > 0 {
                        let delta = amount.abs_diff(baseline);
                        if delta * 100 > baseline * 50 {
                            flagged = true;
                            reason = Some(format!(
                                "amount {amount} deviates >50% from historical baseline {baseline}"
                            ));
                        }
                    }
                }
            }

            lines.push(PayrollLine {
                employee_id: emp.employee_id.clone(),
                amount_cents: amount,
                flagged,
                flag_reason: reason,
                ..Default::default()
            });
        }

        if total > cap_cents {
            return Err(format!(
                "compute-payroll: cycle total {total} exceeds batch_cap_cents {cap_cents} — reduce roster or raise cap"
            ));
        }

        record.status = "computed".to_string();
        record.org_id = Some(req.org_id.clone());
        record.pay_period_start = Some(req.pay_period_start.clone());
        record.pay_period_end = Some(req.pay_period_end.clone());
        record.total_cents = total;
        record.lines = lines.clone();
        kv_put_json("cycles", &req.cycle_id, &record)?;

        let _ = logging::info(&format!(
            "compute-payroll: cycle {} computed, total_cents={total}, lines={}",
            req.cycle_id,
            record.lines.len()
        ));

        #[derive(serde::Serialize)]
        struct Resp {
            cycle_id: String,
            total_cents: u64,
            lines: Vec<PayrollLine>,
        }
        serde_json::to_vec(&Resp {
            cycle_id: req.cycle_id,
            total_cents: total,
            lines,
        })
        .map_err(|e| e.to_string())
    }

    pub fn execute_disbursement(req: CycleIdReq) -> Result<Vec<u8>, String> {
        let mut record: CycleRecord = kv_get_json("cycles", &req.cycle_id)?
            .ok_or_else(|| format!("execute-disbursement: unknown cycle {}", req.cycle_id))?;
        if record.status != "computed" {
            return Err(format!(
                "execute-disbursement: cycle {} is in status '{}', expected 'computed'",
                req.cycle_id, record.status
            ));
        }

        let employees = list_employees()?;
        let by_id: BTreeMap<&str, &Employee> =
            employees.iter().map(|e| (e.employee_id.as_str(), e)).collect();
        let api_key = get_bank_api_key()?;
        let calling_user_hex = tenant_context::calling_user_did().map(hex::encode);

        for line in record.lines.iter_mut() {
            if line.flagged {
                continue; // escalated, not disbursed
            }
            let emp = match by_id.get(line.employee_id.as_str()) {
                Some(e) => *e,
                None => {
                    line.disbursement_error = Some("employee record missing at disbursement time".to_string());
                    continue;
                }
            };

            let result = match emp.pii_source.as_str() {
                "profile" => disburse_via_placeholder(emp, line.amount_cents, &api_key, calling_user_hex.as_deref()),
                _ => disburse_synthetic(emp, line.amount_cents, &api_key),
            };

            match result {
                Ok((bank_ref, pii_mode)) => {
                    line.disbursed = true;
                    line.bank_ref = Some(bank_ref);
                    line.pii_mode = Some(pii_mode);
                }
                Err(e) => {
                    let _ = logging::error(&format!(
                        "execute-disbursement: employee {} failed: {e}",
                        line.employee_id
                    ));
                    line.disbursement_error = Some(e);
                }
            }
        }

        record.status = "disbursed".to_string();
        kv_put_json("cycles", &req.cycle_id, &record)?;

        #[derive(serde::Serialize)]
        struct Resp {
            cycle_id: String,
            results: Vec<PayrollLine>,
        }
        serde_json::to_vec(&Resp {
            cycle_id: req.cycle_id,
            results: record.lines,
        })
        .map_err(|e| e.to_string())
    }

    /// Profile-linked employee: the bank account/routing numbers live ONLY in
    /// the calling user's T3N profile. We template `{{profile.<field>}}`
    /// markers and let the host resolve + substitute them — this contract's
    /// WASM never sees the plaintext digits.
    fn disburse_via_placeholder(
        emp: &Employee,
        amount_cents: u64,
        api_key: &str,
        calling_user_hex: Option<&str>,
    ) -> Result<(String, String), String> {
        let expected = emp
            .employee_did_hex
            .as_deref()
            .ok_or("profile-linked employee missing employee_did_hex")?;
        if calling_user_hex != Some(expected) {
            return Err(
                "execute-disbursement must be invoked with pii_did == this employee's own DID for placeholder resolution"
                    .to_string(),
            );
        }

        // NOTE: Terminal3's current user-profile schema only accepts a closed
        // Level-1 field set (first_name/last_name/.../ssn/address/...) — a
        // custom `bank_account_number` field is rejected by `submitUserInput`
        // ("UnrecognizedKeys"), see docs/BUGS.md. This demo repurposes the
        // existing `ssn` / `address` fields to carry the demo bank
        // account/routing numbers; the substitution mechanism itself
        // (host-resolved `{{profile.<field>}}`, contract never sees the
        // plaintext) is identical to what a real `bank_account_number` field
        // would use once the schema supports it.
        let body = serde_json::json!({
            "employee_id": emp.employee_id,
            "amount_cents": amount_cents,
            "account_number": "{{profile.ssn}}",
            "routing_number": "{{profile.address}}",
        });

        let _ = logging::info(&format!(
            "execute-disbursement: PII-safe placeholder disbursement for {}",
            emp.employee_id
        ));

        let resp = hwp::call(&hwp::Request {
            method: hwp::Verb::Post,
            url: bank_url(),
            headers: Some(bank_headers(api_key)),
            payload: Some(serde_json::to_vec(&body).map_err(|e| e.to_string())?),
        })
        .map_err(|e| format!("bank transfer (placeholder): {}", format_hwp_error(e)))?;

        parse_bank_response(resp.code, &resp.payload).map(|r| (r, "placeholder".to_string()))
    }

    /// Synthetic demo employee: fake bank fields already sit in KV (no real
    /// human, no real PII). Plain `http` — deliberately NOT claimed as
    /// PII-protected.
    fn disburse_synthetic(emp: &Employee, amount_cents: u64, api_key: &str) -> Result<(String, String), String> {
        let account = emp
            .bank_account_number
            .clone()
            .ok_or("synthetic employee missing bank_account_number")?;
        let routing = emp
            .bank_routing_number
            .clone()
            .ok_or("synthetic employee missing bank_routing_number")?;

        let body = serde_json::json!({
            "employee_id": emp.employee_id,
            "amount_cents": amount_cents,
            "account_number": account,
            "routing_number": routing,
        });

        let _ = logging::info(&format!(
            "execute-disbursement: synthetic (non-PII-protected) disbursement for {}",
            emp.employee_id
        ));

        let resp = http_iface::call(&http_iface::Request {
            method: http_iface::Verb::Post,
            url: bank_url(),
            headers: Some(bank_headers(api_key)),
            payload: Some(serde_json::to_vec(&body).map_err(|e| e.to_string())?),
        })
        .map_err(|e| format!("bank transfer (synthetic): {e}"))?;

        parse_bank_response(resp.code, &resp.payload).map(|r| (r, "synthetic_plain".to_string()))
    }

    fn parse_bank_response(code: u16, payload: &[u8]) -> Result<String, String> {
        if !(200..300).contains(&code) {
            return Err(format!("bank endpoint returned HTTP {code}"));
        }
        let v: serde_json::Value = serde_json::from_slice(payload).unwrap_or(serde_json::Value::Null);
        let bank_ref = v
            .get("bank_ref")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("ref-{code}"));
        Ok(bank_ref)
    }

    fn bank_url() -> String {
        // Mock bank stand-in: httpbin's /post echoes the request back as JSON
        // over real HTTPS, so this exercises the genuine egress + (for the
        // profile line) placeholder-substitution path end-to-end without
        // depending on a bank sandbox account. See README "Mock bank" section.
        option_env!("Z_PAYROLL_BANK_URL")
            .unwrap_or("https://httpbin.org/post")
            .to_string()
    }

    fn bank_headers(api_key: &str) -> Vec<(String, String)> {
        vec![
            ("Authorization".to_string(), format!("Bearer {api_key}")),
            ("Content-Type".to_string(), "application/json".to_string()),
        ]
    }

    fn format_hwp_error(e: hwp::HttpError) -> String {
        match e {
            hwp::HttpError::EgressDenied(host) => format!("egress denied for host {host}"),
            hwp::HttpError::PlaceholderDenied(marker) => format!("placeholder not permitted: {marker}"),
            hwp::HttpError::PlaceholderUnknown(field) => format!("user profile missing field: {field}"),
            hwp::HttpError::PlaceholderNoUserContext => {
                "no user context bound for placeholder resolution — call with pii_did set".to_string()
            }
            hwp::HttpError::UpstreamError(reason) => format!("upstream: {reason}"),
        }
    }

    pub fn finalize_audit(req: CycleIdReq) -> Result<Vec<u8>, String> {
        let mut record: CycleRecord = kv_get_json("cycles", &req.cycle_id)?
            .ok_or_else(|| format!("finalize-audit: unknown cycle {}", req.cycle_id))?;
        if record.status != "disbursed" {
            return Err(format!(
                "finalize-audit: cycle {} is in status '{}', expected 'disbursed'",
                req.cycle_id, record.status
            ));
        }

        let disbursed_count = record.lines.iter().filter(|l| l.disbursed).count() as u32;
        let flagged_count = record.lines.iter().filter(|l| l.flagged).count() as u32;

        let entry = AuditEntry {
            cycle_id: req.cycle_id.clone(),
            finalized_at_secs: now_secs(),
            total_cents: record.total_cents,
            employee_count: record.lines.len() as u32,
            disbursed_count,
            flagged_count,
            lines: record.lines.clone(),
        };
        kv_put_json("audit_cycles", &req.cycle_id, &entry)?;

        record.status = "finalized".to_string();
        kv_put_json("cycles", &req.cycle_id, &record)?;

        let _ = logging::info(&format!(
            "finalize-audit: cycle {} finalized, disbursed={disbursed_count} flagged={flagged_count}",
            req.cycle_id
        ));

        serde_json::to_vec(&entry).map_err(|e| e.to_string())
    }

    pub fn submit_escalations(req: CycleIdReq) -> Result<Vec<u8>, String> {
        let mut record: CycleRecord = kv_get_json("cycles", &req.cycle_id)?
            .ok_or_else(|| format!("submit-escalations: unknown cycle {}", req.cycle_id))?;

        let escalations: Vec<Escalation> = record
            .lines
            .iter()
            .filter(|l| l.flagged)
            .map(|l| Escalation {
                employee_id: l.employee_id.clone(),
                reason: l.flag_reason.clone().unwrap_or_else(|| "flagged".to_string()),
            })
            .collect();

        record.escalations = escalations.clone();
        kv_put_json("cycles", &req.cycle_id, &record)?;

        let _ = logging::info(&format!(
            "submit-escalations: cycle {} has {} escalation(s)",
            req.cycle_id,
            escalations.len()
        ));

        #[derive(serde::Serialize)]
        struct Resp {
            cycle_id: String,
            escalations: Vec<Escalation>,
        }
        serde_json::to_vec(&Resp {
            cycle_id: req.cycle_id,
            escalations,
        })
        .map_err(|e| e.to_string())
    }

    pub fn get_audit_entry(req: CycleIdReq) -> Result<Vec<u8>, String> {
        let entry: AuditEntry = kv_get_json("audit_cycles", &req.cycle_id)?
            .ok_or_else(|| format!("get-audit-entry: cycle {} has no finalized audit entry", req.cycle_id))?;
        serde_json::to_vec(&entry).map_err(|e| e.to_string())
    }

    pub fn list_audit_cycles(req: ListCyclesReq) -> Result<Vec<u8>, String> {
        let limit = req.limit.unwrap_or(20).clamp(1, 200) as usize;
        let map = tenant_map("audit_cycles");
        let end = vec![0xffu8; 64];
        let rows = kv_store::scan(&map, &[], &end, 1000).map_err(|e| format!("kv scan {map}: {e}"))?;

        let mut entries: Vec<AuditEntry> = rows
            .into_iter()
            .filter_map(|(_, v)| serde_json::from_slice::<AuditEntry>(&v).ok())
            .collect();
        entries.sort_by(|a, b| b.finalized_at_secs.cmp(&a.finalized_at_secs));
        entries.truncate(limit);

        #[derive(serde::Serialize)]
        struct Resp {
            cycles: Vec<AuditEntry>,
        }
        serde_json::to_vec(&Resp { cycles: entries }).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_payroll_bad_input_returns_err() {
        let result = compute_payroll(b"not json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bad input"));
    }

    #[test]
    fn validate_credentials_bad_input_returns_err() {
        let result = validate_credentials(b"not json");
        assert!(result.is_err());
    }

    #[test]
    fn native_target_stub_errors_are_explicit() {
        let input = serde_json::to_vec(&serde_json::json!({ "cycle_id": "c1" })).unwrap();
        let result = validate_credentials(&input);
        assert!(result.unwrap_err().contains("only implemented on the wasm32 target"));
    }
}
