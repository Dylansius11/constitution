use borsh::{
    BorshSchema,
    schema::{Definition, Fields},
    schema_container_of,
};
use constitution_types::*;

#[test]
fn certificate_borsh_schema_preserves_raw_field_declarations_and_order() {
    let schema = schema_container_of::<Certificate>();
    let Some(Definition::Struct {
        fields: Fields::NamedFields(fields),
    }) = schema.get_definition(schema.declaration())
    else {
        panic!("certificate must be a named-field struct");
    };
    let expected = [
        ("domain", <[u8; 16]>::declaration()),
        ("gate", AccountId::declaration()),
        ("proposal_id", u64::declaration()),
        ("attempt", u32::declaration()),
        ("chain_id", u64::declaration()),
        ("proxy", <[u8; 20]>::declaration()),
        ("executor", <[u8; 20]>::declaration()),
        ("current_impl", <[u8; 20]>::declaration()),
        ("current_impl_codehash", <[u8; 32]>::declaration()),
        ("new_impl", <[u8; 20]>::declaration()),
        ("new_impl_codehash", <[u8; 32]>::declaration()),
        ("init_calldata_hash", <[u8; 32]>::declaration()),
        ("source_commitment", <[u8; 32]>::declaration()),
        ("rule_set_hash", <[u8; 32]>::declaration()),
        ("verifier_pubkey", <[u8; 32]>::declaration()),
        ("verifier_release_hash", <[u8; 32]>::declaration()),
        ("evidence_block_number", u64::declaration()),
        ("evidence_block_hash", <[u8; 32]>::declaration()),
        ("evidence_root", <[u8; 32]>::declaration()),
        ("ai", AiReview::declaration()),
        ("verdict", Verdict::declaration()),
        ("issued_at_s", u64::declaration()),
        ("expires_at_s", u64::declaration()),
    ];
    assert_eq!(
        fields,
        &expected
            .into_iter()
            .map(|(name, declaration)| (name.to_owned(), declaration))
            .collect::<Vec<_>>()
    );
    for (_, declaration) in fields {
        assert!(
            schema.get_definition(declaration).is_some(),
            "missing {declaration}"
        );
    }
    assert!(
        !schema
            .definitions()
            .any(|(name, _)| name == "Hex20" || name == "Hex32")
    );
    schema.validate().unwrap();
}

#[test]
fn wrapper_borsh_schemas_equal_the_raw_encodings() {
    assert_eq!(
        schema_container_of::<Hex20>(),
        schema_container_of::<[u8; 20]>()
    );
    assert_eq!(
        schema_container_of::<Hex32>(),
        schema_container_of::<[u8; 32]>()
    );
    assert_eq!(
        schema_container_of::<HexBytes>(),
        schema_container_of::<Vec<u8>>()
    );
    assert_eq!(
        schema_container_of::<U128Str>(),
        schema_container_of::<u128>()
    );
    assert_eq!(
        schema_container_of::<Base64<32>>(),
        schema_container_of::<[u8; 32]>()
    );
    assert_eq!(
        schema_container_of::<Base64<64>>(),
        schema_container_of::<[u8; 64]>()
    );
    assert_eq!(
        schema_container_of::<Base64Bytes>(),
        schema_container_of::<Vec<u8>>()
    );
}

#[test]
fn every_shared_storage_schema_is_available_and_valid() {
    fn validate<T: BorshSchema>() {
        schema_container_of::<T>().validate().unwrap();
    }
    validate::<Config>();
    validate::<Constitution>();
    validate::<Params>();
    validate::<Roles>();
    validate::<VerifierRecord>();
    validate::<ProposalState>();
    validate::<Proposal>();
    validate::<Rejection>();
    validate::<EvmTx>();
    validate::<ProposeArgs>();
    validate::<ChangeKind>();
    validate::<ChangeStatus>();
    validate::<PendingChange>();
    validate::<SignTarget>();
    validate::<AiReview>();
    validate::<Verdict>();
}
