use risc0_zkvm::guest::env;

use std::io::Read;

use p384::ecdsa::signature::hazmat::PrehashVerifier;
use p384::ecdsa::signature::Verifier;
use p384::ecdsa::Signature;
use p384::ecdsa::VerifyingKey;
use sha2::Digest;
use x509_cert::der::Decode;

// Design notes:
// Generally, it asserts a specific structure instead of parsing everything in a generic fashion.
// Helps keep the proving time low at the cost of being less flexible towards structure changes.
// Skips processing certificate extensions and subject/issuers. Verifies only signatures, expiry.

fn main() {
    // read the attestation
    let mut attestation = Vec::<u8>::new();
    env::stdin().read_to_end(&mut attestation).unwrap();

    println!(
        "Attestation: {} bytes: {:?}",
        attestation.len(),
        attestation
    );

    verify(&attestation, env::commit_slice);

    println!("Done!");
}

fn verify(attestation: &[u8], commit_slice: impl Fn(&[u8])) {
    // assert initial fields
    assert_eq!(
        attestation[0..8],
        [
            0x84, // the COSE structure is an array of size 4
            0x44, 0xa1, 0x01, 0x38, 0x22, // protected header, specifying P384 signature
            0xa0, // empty unprotected header
            0x59, // payload size of 2 bytes follows
        ]
    );

    // get payload size
    let payload_size = u16::from_be_bytes([attestation[8], attestation[9]]) as usize;
    println!("Payload size: {payload_size}");

    // assert total size
    assert_eq!(attestation.len(), 10 + payload_size + 98);

    // payload should be in attestation[10..10 + payload_size]
    // signature should be in attestation[12 + payload_size..108 + payload_size]

    // skip fields and simply assert length
    assert_eq!(
        attestation[10..12],
        [
            0xa9, // attestation doc payload is map of size 9
            // expected keys: module_id, digest, timestamp, pcrs, certificate, cabundle,
            // public_key, user_data, nonce
            0x69, // text of size 9, "module_id" key
        ]
    );
    assert_eq!(attestation[21], 0x78); // text with one byte length follows

    // skip to after the module id
    let mut offset = 23 + attestation[22] as usize;

    assert_eq!(attestation[offset], 0x66); // text of size 6, "digest" key
    assert_eq!(attestation[offset + 7], 0x66); // text of size 6, "SHA384" value

    // assert timestamp key
    assert_eq!(attestation[offset + 14], 0x69); // text of size 9
    assert_eq!(&attestation[offset + 15..offset + 24], b"timestamp");
    // commit the timestamp value
    assert_eq!(attestation[offset + 24], 0x1b); // unsigned int, size 8
    println!("Timestamp: {:?}", &attestation[offset + 25..offset + 33]);
    commit_slice(&attestation[offset + 25..offset + 33]);

    // extract timestamp for expiry checks, convert from milliseconds to seconds
    let timestamp =
        u64::from_be_bytes(attestation[offset + 25..offset + 33].try_into().unwrap()) / 1000;

    // assert pcrs key
    assert_eq!(attestation[offset + 33], 0x64); // text of size 4
    assert_eq!(&attestation[offset + 34..offset + 38], b"pcrs");
    // commit pcrs 0, 1 and 2
    assert_eq!(attestation[offset + 38], 0xb0); // pcrs is a map of size 16

    offset += 39;
    assert_eq!(
        attestation[offset..offset + 3],
        [
            0x00, // pcr number
            0x58, // bytes with one byte length follows
            0x30, // 48 length
        ]
    );
    println!("PCR0: {:?}", &attestation[offset + 3..offset + 51]);
    commit_slice(&attestation[offset + 3..offset + 51]);

    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x01, 0x58, 0x30]);
    println!("PCR1: {:?}", &attestation[offset + 3..offset + 51]);
    commit_slice(&attestation[offset + 3..offset + 51]);

    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x02, 0x58, 0x30]);
    println!("PCR2: {:?}", &attestation[offset + 3..offset + 51]);
    commit_slice(&attestation[offset + 3..offset + 51]);

    // skip rest of the pcrs, 3 to 15
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x03, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x04, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x05, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x06, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x07, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x08, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x09, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x0a, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x0b, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x0c, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x0d, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x0e, 0x58, 0x30]);
    offset += 51;
    assert_eq!(attestation[offset..offset + 3], [0x0f, 0x58, 0x30]);
    offset += 51;
    println!("Skipped rest of the pcrs");

    // assert certificate key
    assert_eq!(attestation[offset], 0x6b); // text of size 11
    assert_eq!(&attestation[offset + 1..offset + 12], b"certificate");
    // get leaf certificate
    assert_eq!(attestation[offset + 12], 0x59); // bytes where two byte length follows
    let leaf_cert_size =
        u16::from_be_bytes([attestation[offset + 13], attestation[offset + 14]]) as usize;
    let leaf_cert_offset = offset + 15;
    let leaf_cert =
        x509_cert::Certificate::from_der(&attestation[offset + 15..offset + 15 + leaf_cert_size])
            .unwrap();
    offset += 15 + leaf_cert_size;

    // assert cabundle key
    assert_eq!(attestation[offset], 0x68); // text of length 8
    assert_eq!(&attestation[offset + 1..offset + 9], b"cabundle");

    // cabundle should be an array, read length
    let chain_size = attestation[offset + 9];
    // just restrict chain size instead of figuring out parsing too much
    // works for tiny field encoded cabundle up to 16 length
    assert!(chain_size > 0x80 && chain_size <= 0x90);
    let chain_size = chain_size - 0x80; // real size is minus 0x80 for bytes type

    // verify certificate chain
    // first certificate in the list is the root certificate
    // last certificate in the list signs the leaf certificate obtained above

    // track start of each section so we know where to proceed from after the block
    offset = offset + 10;
    {
        // start with the root cert

        // parse root cert
        assert_eq!(attestation[offset], 0x59); // bytes where two byte length follows
        let size = u16::from_be_bytes([attestation[offset + 1], attestation[offset + 2]]) as usize;

        // cert with the pubkey, start with the root
        let mut parent_cert =
            x509_cert::Certificate::from_der(&attestation[offset + 3..offset + 3 + size]).unwrap();
        // assert parent cert expiry
        assert!(
            parent_cert
                .tbs_certificate
                .validity
                .not_before
                .to_unix_duration()
                .as_secs()
                < timestamp
        );
        assert!(
            parent_cert
                .tbs_certificate
                .validity
                .not_after
                .to_unix_duration()
                .as_secs()
                > timestamp
        );

        // commit the root pubkey
        let pubkey = parent_cert
            .tbs_certificate
            .subject_public_key_info
            .subject_public_key
            .raw_bytes();
        println!(
            "Root certificate public key: {} bytes: {:?}",
            pubkey.len(),
            pubkey
        );
        // assert that the pubkey size is 97 in case it changes later
        assert_eq!(pubkey.len(), 97);
        assert_eq!(pubkey[0], 0x04);
        commit_slice(&pubkey[1..]);

        // start of next cert that is to be verified
        offset = offset + 3 + size;

        for _ in 0..chain_size - 1 {
            // parse child cert
            assert_eq!(attestation[offset], 0x59); // bytes where two byte length follows
            let size =
                u16::from_be_bytes([attestation[offset + 1], attestation[offset + 2]]) as usize;

            // parse the next cert and get the public key
            let child_cert =
                x509_cert::Certificate::from_der(&attestation[offset + 3..offset + 3 + size])
                    .unwrap();
            // assert cert expiry
            assert!(
                child_cert
                    .tbs_certificate
                    .validity
                    .not_before
                    .to_unix_duration()
                    .as_secs()
                    < timestamp
            );
            assert!(
                child_cert
                    .tbs_certificate
                    .validity
                    .not_after
                    .to_unix_duration()
                    .as_secs()
                    > timestamp
            );

            // verify signature
            // the tbs cert is already available in DER form in the attestation, use that
            assert_eq!(attestation[offset + 3], 0x30); // ASN.1 Sequence
            assert_eq!(attestation[offset + 4], 0x82); // two byte length follows
            assert_eq!(attestation[offset + 7], 0x30); // ASN.1 Sequence
            assert_eq!(attestation[offset + 8], 0x82); // two byte length follows
            let cert_size =
                u16::from_be_bytes([attestation[offset + 9], attestation[offset + 10]]) as usize;
            let msg = &attestation[offset + 7..offset + 11 + cert_size];
            let sig = Signature::from_der(&child_cert.signature.raw_bytes()).unwrap();
            let pubkey = parent_cert
                .tbs_certificate
                .subject_public_key_info
                .subject_public_key
                .raw_bytes();
            let vkey = VerifyingKey::from_sec1_bytes(&pubkey).unwrap();
            vkey.verify(&msg, &sig).unwrap();

            // set up for next iteration
            parent_cert = child_cert;
            offset = offset + 3 + size;
        }

        // assert leaf cert expiry
        assert!(
            leaf_cert
                .tbs_certificate
                .validity
                .not_before
                .to_unix_duration()
                .as_secs()
                < timestamp
        );
        assert!(
            leaf_cert
                .tbs_certificate
                .validity
                .not_after
                .to_unix_duration()
                .as_secs()
                > timestamp
        );

        // verify the leaf cert with the last cert in the chain
        // the tbs cert is already available in DER form in the attestation, use that
        assert_eq!(attestation[leaf_cert_offset], 0x30); // ASN.1 Sequence
        assert_eq!(attestation[leaf_cert_offset + 1], 0x82); // two byte length follows
        assert_eq!(attestation[leaf_cert_offset + 4], 0x30); // ASN.1 Sequence
        assert_eq!(attestation[leaf_cert_offset + 5], 0x82); // two byte length follows
        let cert_size = u16::from_be_bytes([
            attestation[leaf_cert_offset + 6],
            attestation[leaf_cert_offset + 7],
        ]) as usize;
        let msg = &attestation[leaf_cert_offset + 4..leaf_cert_offset + 8 + cert_size];
        let sig = Signature::from_der(&leaf_cert.signature.raw_bytes()).unwrap();
        let pubkey = parent_cert
            .tbs_certificate
            .subject_public_key_info
            .subject_public_key
            .raw_bytes();
        let vkey = VerifyingKey::from_sec1_bytes(&pubkey).unwrap();
        vkey.verify(&msg, &sig).unwrap();
    }

    // assert public_key key
    assert_eq!(attestation[offset], 0x6a); // text of size 10
    assert_eq!(&attestation[offset + 1..offset + 11], b"public_key");

    offset = offset + 11;

    // commit public key
    let pubkey_len = if attestation[offset] >= 0x40 && attestation[offset] <= 0x57 {
        // length is part of type byte
        let len = attestation[offset] - 0x40;

        // set offset to start of pubkey
        offset += 1;

        // return len
        len as usize
    } else {
        // only allow one byte length
        assert_eq!(attestation[offset], 0x58);
        let len = attestation[offset + 1] as usize;

        // set offset to start of pubkey
        offset += 2;

        // return len
        len
    };

    println!(
        "Public key: {pubkey_len} bytes: {:?}",
        &attestation[offset..offset + pubkey_len]
    );
    commit_slice(&[pubkey_len as u8]);
    commit_slice(&attestation[offset..offset + pubkey_len]);

    offset = offset + pubkey_len;

    // assert user_data key
    assert_eq!(attestation[offset], 0x69); // text of size 9
    assert_eq!(&attestation[offset + 1..offset + 10], b"user_data");
    // commit user data
    // handle cases up to 65536 size
    let (user_data_size, user_data) = if attestation[offset + 10] == 0xf6 {
        // empty
        (0, [].as_slice())
    } else if attestation[offset + 10] >= 0x40 && attestation[offset + 10] <= 0x57 {
        // length is part of type byte
        let size = (attestation[offset + 10] - 0x40) as u16;

        (size, &attestation[offset + 11..offset + 11 + size as usize])
    } else if attestation[offset + 10] == 0x58 {
        // one byte length follows
        let size = attestation[offset + 11] as u16;

        (size, &attestation[offset + 12..offset + 12 + size as usize])
    } else {
        // only allow 2 byte lengths as max
        // technically, this is already enforced by COSE doc size parsing
        assert_eq!(attestation[offset + 10], 0x59);

        let size = u16::from_be_bytes([attestation[offset + 11], attestation[offset + 12]]);

        (size, &attestation[offset + 13..offset + 13 + size as usize])
    };
    println!("User data: {} bytes: {:?}", user_data_size, user_data);
    // commit 2 byte length, then data
    commit_slice(&user_data_size.to_be_bytes());
    commit_slice(user_data);

    // prepare COSE verification hash
    let mut hasher = sha2::Sha384::new();
    // array with 4 elements
    hasher.update(&[0x84]);
    // context field length
    hasher.update(&[0x6a]);
    // context field
    hasher.update("Signature1");
    // body_protected
    hasher.update(&[0x44, 0xa1, 0x01, 0x38, 0x22]);
    // empty aad
    hasher.update(&[0x40]);
    // payload length
    hasher.update(&[0x59, attestation[8], attestation[9]]);
    // payload
    hasher.update(&attestation[10..10 + payload_size]);
    let hash = hasher.finalize();

    // verify signature
    // signature size
    assert_eq!(attestation[payload_size + 10], 0x58); // bytes where one byte length follows
    assert_eq!(attestation[payload_size + 11], 0x60); // 96 length

    let leaf_cert_pubkey = leaf_cert
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes()
        .to_owned();
    let verifying_key = VerifyingKey::from_sec1_bytes(&leaf_cert_pubkey).unwrap();
    let r: [u8; 48] = attestation[12 + payload_size..60 + payload_size]
        .try_into()
        .unwrap();
    let s: [u8; 48] = attestation[60 + payload_size..108 + payload_size]
        .try_into()
        .unwrap();
    let signature = Signature::from_scalars(r, s).unwrap();

    verifying_key.verify_prehash(&hash, &signature).unwrap();
}
