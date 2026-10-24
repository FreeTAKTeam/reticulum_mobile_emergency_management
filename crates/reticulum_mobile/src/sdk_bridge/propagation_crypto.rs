#[derive(Debug, Eq, PartialEq)]
enum PropagationPayloadDecryptError {
    PayloadTooShort { len: usize },
    DestinationMismatch { expected: String, actual: String },
    DecryptFailed,
}

impl fmt::Display for PropagationPayloadDecryptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooShort { len } => {
                write!(f, "payload too short for propagation transient len={len}")
            }
            Self::DestinationMismatch { expected, actual } => write!(
                f,
                "destination prefix mismatch expected={expected} actual={actual}"
            ),
            Self::DecryptFailed => write!(f, "propagation transient decrypt failed"),
        }
    }
}

fn decrypt_local_propagated_wire(
    identity: &PrivateIdentity,
    destination_hash: &AddressHash,
    transient_payload: &[u8],
) -> Result<Vec<u8>, PropagationPayloadDecryptError> {
    if transient_payload.len() <= 16 + 32 {
        return Err(PropagationPayloadDecryptError::PayloadTooShort {
            len: transient_payload.len(),
        });
    }
    if &transient_payload[..16] != destination_hash.as_slice() {
        return Err(PropagationPayloadDecryptError::DestinationMismatch {
            expected: destination_hash.to_hex_string(),
            actual: hex::encode(&transient_payload[..16]),
        });
    }

    for strip_stamp in [false, true] {
        let payload = if strip_stamp {
            if transient_payload.len() <= 16 + 32 + 32 {
                continue;
            }
            &transient_payload[..transient_payload.len() - 32]
        } else {
            transient_payload
        };

        let ciphertext = &payload[16..];
        if let Ok(decrypted) =
            decrypt_propagation_ciphertext(identity, destination_hash, ciphertext)
        {
            let mut wire = Vec::with_capacity(16 + decrypted.len());
            wire.extend_from_slice(destination_hash.as_slice());
            wire.extend_from_slice(decrypted.as_slice());
            return Ok(wire);
        }
    }

    Err(PropagationPayloadDecryptError::DecryptFailed)
}

fn decrypt_propagation_ciphertext(
    identity: &PrivateIdentity,
    destination_hash: &AddressHash,
    ciphertext: &[u8],
) -> Result<Vec<u8>, PropagationPayloadDecryptError> {
    if ciphertext.len() <= 32 {
        return Err(PropagationPayloadDecryptError::DecryptFailed);
    }
    let Ok(ephemeral_key) = <[u8; 32]>::try_from(&ciphertext[..32]) else {
        return Err(PropagationPayloadDecryptError::DecryptFailed);
    };
    let public_key = PublicKey::from(ephemeral_key);
    let token = &ciphertext[32..];

    let mut salts = Vec::with_capacity(2);
    salts.push(identity.address_hash().as_slice());
    if destination_hash.as_slice() != identity.address_hash().as_slice() {
        salts.push(destination_hash.as_slice());
    }

    for salt in salts {
        let derived_key = identity.derive_key(&public_key, Some(salt));
        let mut plaintext = vec![0u8; token.len()];
        let Ok(decrypted_len) = identity
            .decrypt(OsRng, token, &derived_key, &mut plaintext)
            .map(|decrypted| decrypted.len())
        else {
            continue;
        };
        plaintext.truncate(decrypted_len);
        return Ok(plaintext);
    }

    Err(PropagationPayloadDecryptError::DecryptFailed)
}
