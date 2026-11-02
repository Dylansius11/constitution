//! Stable rule, reason and panic-code vocabulary from SMART-CONTRACT.

macro_rules! numbered_enum {
    ($name:ident, $repr:ty, {$($variant:ident = $value:literal),+ $(,)?}) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[repr($repr)]
        pub enum $name { $($variant = $value),+ }
        impl TryFrom<$repr> for $name {
            type Error = $repr;
            fn try_from(value: $repr) -> Result<Self, Self::Error> {
                match value { $($value => Ok(Self::$variant),)+ _ => Err(value) }
            }
        }
    }
}

numbered_enum!(RuleId, u8, {
    PreCheck = 0, StorageLayout = 1, Solvency = 2, OpcodeAndSlotFilter = 3,
    Initializer = 4, DifferentialReplay = 5, AiReview = 6,
});
numbered_enum!(ReasonCode, u16, {
    SourceHashMismatch = 1,
    CompiledRuntimeHashMismatch = 2,
    CurrentImplementationMismatch = 3,
    CurrentSourceRuntimeMismatch = 4,
    IncompatibleLayout = 10,
    ReferenceLayoutMissing = 11,
    BoundedCounterexampleFound = 20,
    FuzzCounterexample = 21,
    AiCounterexampleReproduced = 22,
    SolverIncomplete = 23,
    ForbiddenOpcode = 30,
    Erc1967SlotConstantPresent = 31,
    SelectorNotAllowlisted = 40,
    ArgumentOutOfSchema = 41,
    BehaviorDiverged = 50,
    WitnessIncomplete = 51,
});

macro_rules! gate_errors {
    ($($variant:ident => $code:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum GateError { $($variant),+ }
        impl GateError {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn code(&self) -> &'static str {
                match self { $(Self::$variant => $code),+ }
            }
        }
    }
}

gate_errors! {
    ZeroDeposit => "E10_ZERO_DEPOSIT",
    InsufficientCredit => "E11_INSUFFICIENT_CREDIT",
    SigningPaused => "E20_SIGNING_PAUSED",
    ActiveProposalExists => "E21_ACTIVE_PROPOSAL_EXISTS",
    InitCalldataTooLong => "E22_INIT_CALLDATA_TOO_LONG",
    ZeroAddress => "E23_ZERO_ADDRESS",
    DuplicateActive => "E24_DUPLICATE_ACTIVE",
    ProposalNotFound => "E30_NOT_FOUND",
    WrongState => "E31_WRONG_STATE",
    NotExpired => "E32_NOT_EXPIRED",
    BadEncoding => "E40_BAD_ENCODING",
    WrongDomain => "E41_WRONG_DOMAIN",
    UnknownVerifier => "E42_UNKNOWN_VERIFIER",
    BadSignature => "E43_BAD_SIGNATURE",
    ProposalExpired => "E44_PROPOSAL_EXPIRED",
    BindingMismatch => "E45_BINDING_MISMATCH",
    CertTime => "E46_CERT_TIME",
    AiReviewRequired => "E47_AI_REVIEW_REQUIRED",
    FeeCap => "E48_FEE_CAP",
    TooManyBumps => "E49_TOO_MANY_BUMPS",
    NotGovernor => "E50_NOT_GOVERNOR",
    InvalidChange => "E51_INVALID_CHANGE",
    ChangeNotFound => "E52_NOT_FOUND",
    TimelockNotElapsed => "E53_TIMELOCK_NOT_ELAPSED",
    NotPending => "E54_NOT_PENDING",
    SignaturesOutstanding => "E55_SIGNATURES_OUTSTANDING",
    NotGuardian => "E56_NOT_GUARDIAN",
    LastVerifier => "E57_LAST_VERIFIER",
}
