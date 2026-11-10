//! Decode a gate-emitted EIP-1559 raw transaction and recover its sender off-chain, so a relay
//! never broadcasts bytes the gate did not produce for the derived EOA (SMART-CONTRACT "signing").
use constitution_types::{Config, EvmTx};
use sha3::{Digest, Keccak256};

#[derive(Debug, PartialEq, Eq)]
pub struct SignedTx {
    pub chain_id: u64,
    pub nonce: u64,
    pub to: [u8; 20],
    pub signing_hash: [u8; 32],
    pub sender: [u8; 20],
    pub tx_hash: [u8; 32],
}

fn keccak(bytes: &[u8]) -> [u8; 32] {
    Keccak256::digest(bytes).into()
}

fn scalar(fields: &rlp::Rlp, index: usize) -> Result<[u8; 32], String> {
    let bytes: Vec<u8> = fields
        .val_at(index)
        .map_err(|e| format!("field {index}: {e}"))?;
    if bytes.is_empty() || bytes.len() > 32 {
        return Err(format!("field {index}: scalar of {} bytes", bytes.len()));
    }
    let mut out = [0; 32];
    out[32 - bytes.len()..].copy_from_slice(&bytes);
    Ok(out)
}

pub fn decode(raw: &[u8]) -> Result<SignedTx, String> {
    let body = raw
        .strip_prefix(&[2])
        .ok_or("not an EIP-1559 (type 2) transaction")?;
    let fields = rlp::Rlp::new(body);
    if !fields.is_list()
        || fields.item_count().map_err(|e| e.to_string())? != 12
        || fields.as_raw().len() != body.len()
    {
        return Err("signed EIP-1559 transaction must be exactly one 12-field RLP list".into());
    }
    let mut stream = rlp::RlpStream::new_list(9);
    for i in 0..9 {
        stream.append_raw(fields.at(i).map_err(|e| e.to_string())?.as_raw(), 1);
    }
    let mut unsigned = vec![2];
    unsigned.extend(stream.out());
    let signing_hash = keccak(&unsigned);
    let to: Vec<u8> = fields.val_at(5).map_err(|e| format!("to: {e}"))?;
    let to: [u8; 20] = to.try_into().map_err(|_| "to is not a 20-byte address")?;
    let parity: u8 = fields.val_at(9).map_err(|e| format!("y parity: {e}"))?;
    let recovery = k256::ecdsa::RecoveryId::from_byte(parity)
        .filter(|_| parity <= 1)
        .ok_or("y parity is not 0 or 1")?;
    let signature =
        k256::ecdsa::Signature::from_scalars(scalar(&fields, 10)?, scalar(&fields, 11)?)
            .map_err(|e| format!("signature: {e}"))?;
    if signature.normalize_s().is_some() {
        return Err("signature has high s (EIP-2)".into());
    }
    let key = k256::ecdsa::VerifyingKey::recover_from_prehash(&signing_hash, &signature, recovery)
        .map_err(|e| format!("sender recovery: {e}"))?;
    let point = key.to_encoded_point(false);
    Ok(SignedTx {
        chain_id: fields.val_at(0).map_err(|e| format!("chain id: {e}"))?,
        nonce: fields.val_at(1).map_err(|e| format!("nonce: {e}"))?,
        to,
        signing_hash,
        sender: keccak(&point.as_bytes()[1..])[12..]
            .try_into()
            .expect("20 bytes"),
        tx_hash: keccak(raw),
    })
}

/// The raw bytes must be exactly what the gate recorded for the derived EOA.
pub fn verify(raw: &[u8], config: &Config, evm: &EvmTx) -> Result<SignedTx, String> {
    let tx = decode(raw)?;
    let hex = |b: &[u8]| format!("0x{}", hex::encode(b));
    if tx.sender != config.derived_eoa {
        return Err(format!(
            "recovered sender {} is not the derived EOA {}",
            hex(&tx.sender),
            hex(&config.derived_eoa)
        ));
    }
    if tx.signing_hash != evm.signing_hash {
        return Err(format!(
            "signing hash {} differs from the gate's {}",
            hex(&tx.signing_hash),
            hex(&evm.signing_hash)
        ));
    }
    if tx.chain_id != config.chain_id || tx.to != config.executor || tx.nonce != evm.nonce {
        return Err(format!(
            "chain {} to {} nonce {} differ from the gate's chain {} executor {} nonce {}",
            tx.chain_id,
            hex(&tx.to),
            tx.nonce,
            config.chain_id,
            hex(&config.executor),
            evm.nonce
        ));
    }
    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn spike() -> (Vec<u8>, Value) {
        let v: Value =
            serde_json::from_str(include_str!("../../../vectors/evm-tx-spike.json")).unwrap();
        let signed = v["cases"][0]["signed"].clone();
        let raw = hex::decode(signed["raw_tx"].as_str().unwrap().trim_start_matches("0x")).unwrap();
        (raw, signed)
    }

    fn config(tx: &SignedTx) -> (Config, EvmTx) {
        let config = Config {
            chain_id: 84532,
            executor: tx.to,
            proxy: [1; 20],
            proxy_admin: [2; 20],
            derived_eoa: tx.sender,
            mpc_account: "v1.signer-prod.testnet".parse().unwrap(),
            mpc_path: "constitution/base-sepolia/v1".into(),
            mpc_domain_id: 0,
            governance_delay_days: constitution_types::DEFAULT_GOVERNANCE_DELAY_DAYS,
        };
        let evm = EvmTx {
            nonce: 0,
            deadline_s: 0,
            gas_limit: 0,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            signing_hash: tx.signing_hash,
            signed_tx: None,
            attempts: 1,
        };
        (config, evm)
    }

    #[test]
    fn real_mpc_signed_spike_transaction_recovers_to_the_derived_eoa() {
        // Signed by v1.signer-prod.testnet for constitution-spike.testnet, mined on a Base Sepolia fork.
        let (raw, signed) = spike();
        let tx = decode(&raw).unwrap();
        assert_eq!(
            hex::encode(tx.sender),
            "8221c9477886cc456d7e1cccc0866e0401b81070"
        );
        assert_eq!(
            format!("0x{}", hex::encode(tx.signing_hash)),
            signed["signing_hash"].as_str().unwrap()
        );
        assert_eq!(
            format!("0x{}", hex::encode(tx.tx_hash)),
            signed["tx_hash"].as_str().unwrap()
        );
        assert_eq!((tx.chain_id, tx.nonce), (84532, 0));
        assert_eq!(
            hex::encode(tx.to),
            "a44d05793c89be410c93647fd08f3523444681ae"
        );
        let (config, evm) = config(&tx);
        assert_eq!(verify(&raw, &config, &evm).unwrap(), tx);
    }

    #[test]
    fn relay_refuses_bytes_the_gate_did_not_record() {
        let (raw, _) = spike();
        let tx = decode(&raw).unwrap();
        let (config, evm) = config(&tx);
        let mut other_eoa = config.clone();
        other_eoa.derived_eoa = [9; 20];
        assert!(
            verify(&raw, &other_eoa, &evm)
                .unwrap_err()
                .contains("not the derived EOA")
        );
        let mut other_hash = evm.clone();
        other_hash.signing_hash = [9; 32];
        assert!(
            verify(&raw, &config, &other_hash)
                .unwrap_err()
                .contains("signing hash")
        );
        let mut other_nonce = evm.clone();
        other_nonce.nonce = 1;
        assert!(
            verify(&raw, &config, &other_nonce)
                .unwrap_err()
                .contains("nonce")
        );
        let mut other_chain = config.clone();
        other_chain.chain_id = 8453;
        assert!(verify(&raw, &other_chain, &evm).is_err());
    }

    #[test]
    fn malformed_raw_transactions_are_rejected() {
        let (raw, _) = spike();
        let mut legacy = raw.clone();
        legacy[0] = 1;
        assert!(decode(&legacy).is_err());
        assert!(decode(&raw[..raw.len() - 1]).is_err());
        let mut trailing = raw.clone();
        trailing.push(0);
        assert!(decode(&trailing).is_err());
        // Flipping one calldata byte changes the hash, so the recovered sender is no longer the EOA.
        let mut tampered = raw.clone();
        let at = raw.len() - 100;
        tampered[at] ^= 1;
        assert_ne!(
            decode(&tampered).map(|t| t.sender).unwrap_or([0; 20]),
            decode(&raw).unwrap().sender
        );
    }
}
