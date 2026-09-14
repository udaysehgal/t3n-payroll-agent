//! z-payroll v0.1.0 — enterprise payroll agent reference contract.
//!
//! Pipeline: `validate-credentials` -> `compute-payroll` -> `execute-disbursement`
//! -> `finalize-audit`, with `submit-escalations` for out-of-policy lines and
//! `get-audit-entry` / `list-audit-cycles` for read-back. See wit/world.wit
//! for the exact wire shapes and README.md for the PII model.
#![warn(clippy::style, missing_debug_implementations)]
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

pub const CONTRACT_VERSION: &str = "0.1.1";

wit_bindgen::generate!({
    world: "payroll",
    path: "wit",
    additional_derives: [
        serde::Deserialize,
        serde::Serialize,
    ],
    generate_all,
});

mod payroll;

struct Component;

#[cfg(target_arch = "wasm32")]
impl exports::z::payroll::contracts::Guest for Component {
    fn validate_credentials(
        req: exports::z::payroll::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input = req.input.ok_or("validate-credentials: missing input")?;
        payroll::validate_credentials(&input)
    }

    fn compute_payroll(
        req: exports::z::payroll::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input = req.input.ok_or("compute-payroll: missing input")?;
        payroll::compute_payroll(&input)
    }

    fn execute_disbursement(
        req: exports::z::payroll::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input = req.input.ok_or("execute-disbursement: missing input")?;
        payroll::execute_disbursement(&input)
    }

    fn finalize_audit(
        req: exports::z::payroll::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input = req.input.ok_or("finalize-audit: missing input")?;
        payroll::finalize_audit(&input)
    }

    fn submit_escalations(
        req: exports::z::payroll::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input = req.input.ok_or("submit-escalations: missing input")?;
        payroll::submit_escalations(&input)
    }

    fn get_audit_entry(
        req: exports::z::payroll::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input = req.input.ok_or("get-audit-entry: missing input")?;
        payroll::get_audit_entry(&input)
    }

    fn list_audit_cycles(
        req: exports::z::payroll::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input = req.input.unwrap_or_default();
        payroll::list_audit_cycles(&input)
    }
}

#[cfg(target_arch = "wasm32")]
export!(Component);

#[cfg(test)]
mod tests {
    use super::CONTRACT_VERSION;

    #[test]
    fn contract_version_is_semver() {
        let parts: Vec<&str> = CONTRACT_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "CONTRACT_VERSION must be MAJOR.MINOR.PATCH");
        for part in parts {
            assert!(part.parse::<u32>().is_ok(), "each part must be a number");
        }
    }
}
