use constitution_types::*;
use proptest::prelude::*;

fn account() -> impl Strategy<Value = AccountId> {
    "[a-z][a-z0-9]{1,30}\\.testnet".prop_map(|s| s.parse().unwrap())
}

fn params() -> impl Strategy<Value = Params> {
    (any::<u128>(), any::<[u64; 6]>(), any::<[u128; 3]>()).prop_map(|(deposit, times, fees)| {
        Params {
            proposal_deposit: deposit,
            proposal_ttl_s: times[0],
            max_cert_ttl_s: times[1],
            sign_validity_s: times[2],
            gas_limit: times[3],
            max_fee_per_gas: fees[0],
            max_priority_fee_per_gas: fees[1],
            fee_cap: fees[2],
            sign_gas_tgas: times[4],
            callback_gas_tgas: times[5],
        }
    })
}

fn evm() -> impl Strategy<Value = EvmTx> {
    (
        any::<[u64; 3]>(),
        any::<[u128; 2]>(),
        any::<[u8; 32]>(),
        proptest::option::of(proptest::collection::vec(any::<u8>(), 0..128)),
        any::<u32>(),
    )
        .prop_map(|(nums, fees, hash, signed, attempts)| EvmTx {
            nonce: nums[0],
            deadline_s: nums[1],
            gas_limit: nums[2],
            max_fee_per_gas: fees[0],
            max_priority_fee_per_gas: fees[1],
            signing_hash: hash,
            signed_tx: signed,
            attempts,
        })
}

fn ai() -> impl Strategy<Value = AiReview> {
    prop_oneof![
        (any::<[u8; 32]>(), any::<u8>()).prop_map(|(evidence_hash, signature_kind)| {
            AiReview::Clean {
                evidence_hash,
                signature_kind,
            }
        }),
        (any::<[u8; 32]>(), any::<[u8; 32]>()).prop_map(|(evidence_hash, counterexample_hash)| {
            AiReview::Vetoed {
                evidence_hash,
                counterexample_hash,
            }
        }),
        Just(AiReview::NotRun),
    ]
}

fn certificate() -> impl Strategy<Value = Certificate> {
    (
        account(),
        any::<[u8; 16]>(),
        any::<[u64; 6]>(),
        any::<u32>(),
        any::<[[u8; 20]; 4]>(),
        any::<[[u8; 32]; 9]>(),
        ai(),
        prop_oneof![
            Just(Verdict::Pass),
            (any::<u8>(), any::<u16>()).prop_map(|(rule_id, reason_code)| Verdict::Reject {
                rule_id,
                reason_code
            })
        ],
    )
        .prop_map(
            |(gate, domain, nums, attempt, addresses, hashes, ai, verdict)| Certificate {
                domain,
                gate,
                proposal_id: nums[0],
                attempt,
                chain_id: nums[1],
                proxy: addresses[0],
                executor: addresses[1],
                current_impl: addresses[2],
                current_impl_codehash: hashes[0],
                new_impl: addresses[3],
                new_impl_codehash: hashes[1],
                init_calldata_hash: hashes[2],
                source_commitment: hashes[3],
                rule_set_hash: hashes[4],
                verifier_pubkey: hashes[5],
                verifier_release_hash: hashes[6],
                evidence_block_number: nums[2],
                evidence_block_hash: hashes[7],
                evidence_root: hashes[8],
                ai,
                verdict,
                issued_at_s: nums[3],
                expires_at_s: nums[4],
            },
        )
}

fn proposal() -> impl Strategy<Value = Proposal> {
    (
        account(),
        any::<[u64; 3]>(),
        any::<[[u8; 32]; 3]>(),
        any::<[[u8; 20]; 2]>(),
        proptest::collection::vec(any::<u8>(), 0..=MAX_INIT_CALLDATA),
        any::<u128>(),
        proptest::sample::select(vec![
            ProposalState::Pending,
            ProposalState::Rejected,
            ProposalState::Expired,
            ProposalState::SignRequested,
            ProposalState::Signed,
            ProposalState::SignFailed,
        ]),
        proptest::option::of(any::<[u8; 32]>()),
        proptest::option::of((any::<u8>(), any::<u16>(), any::<[u8; 32]>()).prop_map(
            |(rule_id, reason_code, evidence_root)| Rejection {
                rule_id,
                reason_code,
                evidence_root,
            },
        )),
        proptest::option::of(evm()),
    )
        .prop_map(
            |(
                proposer,
                nums,
                hashes,
                addresses,
                init_calldata,
                reserved,
                state,
                certificate_hash,
                rejection,
                evm,
            )| Proposal {
                id: nums[0],
                proposer,
                created_at_ns: nums[1],
                expires_at_ns: nums[2],
                rule_set_hash: hashes[0],
                new_impl: addresses[0],
                new_impl_codehash: hashes[1],
                expected_current_impl: addresses[1],
                init_calldata,
                source_commitment: hashes[2],
                reserved,
                state,
                certificate_hash,
                rejection,
                evm,
            },
        )
}

fn change_kind() -> impl Strategy<Value = ChangeKind> {
    prop_oneof![
        (any::<[u8; 32]>(), ".{0,256}", any::<bool>()).prop_map(
            |(rule_set_hash, manifest_uri, ai_required)| ChangeKind::SetRuleSet {
                rule_set_hash,
                manifest_uri,
                ai_required
            }
        ),
        (any::<[u8; 32]>(), any::<[u8; 32]>(), ".{0,256}").prop_map(
            |(pubkey, release_hash, attestation_uri)| ChangeKind::AddVerifier {
                pubkey,
                release_hash,
                attestation_uri
            }
        ),
        any::<[u8; 32]>().prop_map(|pubkey| ChangeKind::RemoveVerifier { pubkey }),
        params().prop_map(|params| ChangeKind::SetParams { params }),
        account().prop_map(|account| ChangeKind::SetGovernor { account }),
        account().prop_map(|account| ChangeKind::SetGuardian { account }),
        any::<[u8; 20]>().prop_map(|new_signer| ChangeKind::MigrateAuthority { new_signer }),
        Just(ChangeKind::Unpause),
    ]
}

fn pending_change() -> impl Strategy<Value = PendingChange> {
    (
        any::<[u64; 3]>(),
        change_kind(),
        account(),
        proptest::sample::select(vec![
            ChangeStatus::Pending,
            ChangeStatus::Applied,
            ChangeStatus::Cancelled,
        ]),
        proptest::option::of(evm()),
    )
        .prop_map(|(nums, kind, scheduled_by, status, evm)| PendingChange {
            id: nums[0],
            kind,
            scheduled_by,
            scheduled_at_ns: nums[1],
            eta_ns: nums[2],
            status,
            evm,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn certificate_borsh_roundtrip(value in certificate()) {
        let encoded = value.to_canonical_bytes().unwrap();
        prop_assert_eq!(Certificate::from_canonical_bytes(&encoded).unwrap(), value);
    }
    #[test]
    fn proposal_borsh_roundtrip(value in proposal()) {
        let encoded = borsh::to_vec(&value).unwrap();
        prop_assert_eq!(borsh::from_slice::<Proposal>(&encoded).unwrap(), value);
    }
    #[test]
    fn pending_change_borsh_roundtrip(value in pending_change()) {
        let encoded = borsh::to_vec(&value).unwrap();
        prop_assert_eq!(borsh::from_slice::<PendingChange>(&encoded).unwrap(), value);
    }
    #[test]
    fn proposal_json_roundtrip(value in proposal()) {
        let encoded = serde_json::to_vec(&value).unwrap();
        prop_assert_eq!(serde_json::from_slice::<Proposal>(&encoded).unwrap(), value);
    }
    #[test]
    fn pending_change_json_roundtrip(value in pending_change()) {
        let encoded = serde_json::to_vec(&value).unwrap();
        prop_assert_eq!(serde_json::from_slice::<PendingChange>(&encoded).unwrap(), value);
    }
    #[test]
    fn certificate_json_roundtrip(value in certificate()) {
        let encoded = serde_json::to_vec(&value).unwrap();
        prop_assert_eq!(serde_json::from_slice::<Certificate>(&encoded).unwrap(), value);
    }
}
