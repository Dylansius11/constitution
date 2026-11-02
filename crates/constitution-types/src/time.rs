//! The sole NEAR nanoseconds / Unix seconds conversion boundary.

/// Floor a NEAR timestamp to Unix seconds.
pub const fn ns_to_s(nanoseconds: u64) -> u64 {
    nanoseconds / 1_000_000_000
}

/// Convert Unix seconds to NEAR nanoseconds, rejecting overflow.
pub const fn s_to_ns(seconds: u64) -> Option<u64> {
    seconds.checked_mul(1_000_000_000)
}
