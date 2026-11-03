use std::fmt;

/// Every CLI error names its cause and the next step (PRD F9).
#[derive(Debug, PartialEq, Eq)]
pub struct Failure {
    pub cause: String,
    pub next: &'static str,
}

impl Failure {
    pub fn new(cause: impl Into<String>, next: &'static str) -> Self {
        Self {
            cause: crate::rpc::strip_urls(&cause.into()),
            next,
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}\nnext: {}", self.cause, self.next)
    }
}
