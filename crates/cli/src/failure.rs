use std::fmt;

/// Every CLI error names its cause and the next step (PRD F9).
#[derive(Debug, PartialEq, Eq)]
pub struct Failure {
    pub cause: String,
    pub next: String,
}

impl Failure {
    pub fn new(cause: impl Into<String>, next: impl Into<String>) -> Self {
        Self {
            cause: crate::rpc::strip_urls(&cause.into()),
            next: next.into(),
        }
    }

    pub fn retry_verifier(cause: impl Into<String>, seconds: u64) -> Self {
        Self::new(
            cause,
            format!(
                "retry after {seconds} seconds with `constitution verifier run --proposal <id> --bundle <bundle>` (add --contract src/File.sol:Name for a directory); use the same --gate, --near-rpc and --verifier-url; do not create a duplicate proposal"
            ),
        )
    }

    /// LOCAL.md: pre-signing faults end only the run; they never reject the proposal.
    pub fn verifier_failed(code: &str) -> Self {
        let (explanation, recovery) = match code {
            "worker_panic" => (
                "worker stopped unexpectedly",
                "ask the verifier operator to inspect the worker panic; ",
            ),
            "sandbox_unavailable" => (
                "sandbox is unavailable",
                "ask the verifier operator to restore the tool sandbox; ",
            ),
            "storage_error" => (
                "storage failed",
                "ask the verifier operator to restore writable storage and free disk space; ",
            ),
            "tool_unavailable" => (
                "required tool is unavailable",
                "ask the verifier operator to restore the pinned verifier tools; ",
            ),
            "tool_output_invalid" => (
                "tool output is invalid",
                "ask the verifier operator to inspect the invalid tool artifacts and committed harness; ",
            ),
            "infrastructure_timeout" => (
                "infrastructure time limit was exceeded",
                "ask the verifier operator to inspect sandbox resource limits; ",
            ),
            "rpc_unavailable" => (
                "RPC is unavailable",
                "ask the verifier operator to restore chain RPC access; ",
            ),
            "committed_data_unavailable" => (
                "committed verifier data is unavailable",
                "ask the verifier operator to restore and validate the committed verifier data; ",
            ),
            "bad_bundle" => (
                "bundle could not be unsealed or validated against the source commitment",
                "check the original canonical bundle and source commitment, then fetch the current identity and re-seal; ",
            ),
            _ => {
                return Self::new(
                    "verifier run failed with a missing or unknown error_code; no certificate; the proposal is still Pending",
                    "ask the verifier operator to inspect the run, then use `constitution verifier run --proposal <id> --bundle <bundle>` with the original bundle",
                );
            }
        };
        let cause = format!(
            "verifier run failed ({code}): {explanation}; no certificate; the proposal is still Pending"
        );
        let mut failure = Self::retry_verifier(cause, 10);
        failure.next.insert_str(0, recovery);
        failure
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}\nnext: {}", self.cause, self.next)
    }
}
