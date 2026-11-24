//! Encrypted, sender-authenticated personal preference bundles. No filesystem,
//! network, Profile or authorization behavior lives here.
use age::{
    armor::{ArmoredReader, ArmoredWriter, Format},
    secrecy::ExposeSecret,
    x25519::{Identity as AgeIdentity, Recipient as AgeRecipient},
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use precept_core::operator::{
    MAX_SYNC_ENVELOPE_BYTES, MAX_SYNC_PAYLOAD_BYTES, PERSONAL_SYNC_FORMAT, PERSONAL_SYNC_VERSION,
    PersonalSyncBundle, PersonalSyncEnvelope, PersonalSyncHeader, PersonalSyncInvitation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidRequest,
    TooLarge,
    Encryption,
    Decryption,
    Authentication,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedIdentity {
    pub owner_id: String,
    pub device_id: String,
    pub age_identity: String,
    pub age_recipient: String,
    pub signing_secret: String,
    pub signing_public: String,
    pub owner_fingerprint: String,
}

impl std::fmt::Debug for GeneratedIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GeneratedIdentity(redacted)")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedSender {
    pub owner_id: String,
    pub device_id: String,
    pub signing_public: String,
}

fn random_hex(bytes: usize) -> Result<String, Error> {
    let mut raw = vec![0_u8; bytes];
    getrandom::fill(&mut raw).map_err(|_| Error::InvalidRequest)?;
    Ok(hex(&raw))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(DIGITS[usize::from(byte >> 4)] as char);
        out.push(DIGITS[usize::from(byte & 15)] as char);
    }
    out
}

fn unhex<const N: usize>(value: &str) -> Result<[u8; N], Error> {
    let bytes = value.as_bytes();
    if bytes.len() != N * 2 || !bytes.iter().all(u8::is_ascii_hexdigit) {
        return Err(Error::InvalidRequest);
    }
    let mut out = [0_u8; N];
    for (index, byte) in out.iter_mut().enumerate() {
        let pair = &bytes[index * 2..index * 2 + 2];
        *byte = u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII hex"), 16)
            .map_err(|_| Error::InvalidRequest)?;
    }
    Ok(out)
}

pub fn owner_fingerprint(owner_id: &str) -> Result<String, Error> {
    if !precept_core::operator::valid_identity(owner_id) {
        return Err(Error::InvalidRequest);
    }
    let digest = Sha256::digest(format!("{PERSONAL_SYNC_FORMAT}\0owner\0{owner_id}").as_bytes());
    Ok(hex(&digest[..16]))
}

pub fn generate_identity() -> Result<GeneratedIdentity, Error> {
    rotate_identity(&random_hex(16)?, &random_hex(16)?)
}

pub fn generate_identity_for_owner(owner_id: &str) -> Result<GeneratedIdentity, Error> {
    rotate_identity(owner_id, &random_hex(16)?)
}

pub fn rotate_identity(owner_id: &str, device_id: &str) -> Result<GeneratedIdentity, Error> {
    if !precept_core::operator::valid_identity(owner_id)
        || !precept_core::operator::valid_identity(device_id)
    {
        return Err(Error::InvalidRequest);
    }
    let age_identity = AgeIdentity::generate()
        .to_string()
        .expose_secret()
        .to_owned();
    let age_recipient = AgeIdentity::from_str(&age_identity)
        .map_err(|_| Error::InvalidRequest)?
        .to_public()
        .to_string();
    let signing_secret = random_hex(32)?;
    let signing_public = hex(SigningKey::from_bytes(&unhex(&signing_secret)?)
        .verifying_key()
        .as_bytes());
    Ok(GeneratedIdentity {
        owner_fingerprint: owner_fingerprint(owner_id)?,
        owner_id: owner_id.to_owned(),
        device_id: device_id.to_owned(),
        age_identity,
        age_recipient,
        signing_secret,
        signing_public,
    })
}

use std::str::FromStr;

fn signing_key(secret: &str) -> Result<SigningKey, Error> {
    Ok(SigningKey::from_bytes(&unhex(secret)?))
}

fn verifying_key(public: &str) -> Result<VerifyingKey, Error> {
    VerifyingKey::from_bytes(&unhex(public)?).map_err(|_| Error::Authentication)
}

fn signature_bytes(header: &PersonalSyncHeader, ciphertext: &str) -> Result<Vec<u8>, Error> {
    let mut out = format!("{PERSONAL_SYNC_FORMAT}\0signature\0").into_bytes();
    serde_json::to_writer(&mut out, header).map_err(|_| Error::InvalidRequest)?;
    out.push(0);
    out.extend_from_slice(ciphertext.as_bytes());
    Ok(out)
}

fn invitation_bytes(invitation: &PersonalSyncInvitation) -> Result<Vec<u8>, Error> {
    let mut out = format!("{PERSONAL_SYNC_FORMAT}\0invitation\0").into_bytes();
    serde_json::to_writer(
        &mut out,
        &(
            &invitation.owner_id,
            &invitation.device_id,
            &invitation.age_recipient,
            &invitation.signing_public,
            &invitation.owner_fingerprint,
        ),
    )
    .map_err(|_| Error::InvalidRequest)?;
    Ok(out)
}

pub fn sign(signing_secret: &str, bytes: &[u8]) -> Result<String, Error> {
    Ok(hex(&signing_key(signing_secret)?.sign(bytes).to_bytes()))
}

pub fn verify(signing_public: &str, bytes: &[u8], signature: &str) -> bool {
    let (Ok(key), Ok(signature)) = (verifying_key(signing_public), unhex(signature)) else {
        return false;
    };
    key.verify(bytes, &Signature::from_bytes(&signature))
        .is_ok()
}

pub fn invitation(identity: &GeneratedIdentity) -> Result<PersonalSyncInvitation, Error> {
    let mut invitation = PersonalSyncInvitation {
        format: PERSONAL_SYNC_FORMAT.into(),
        version: PERSONAL_SYNC_VERSION,
        owner_id: identity.owner_id.clone(),
        device_id: identity.device_id.clone(),
        age_recipient: identity.age_recipient.clone(),
        signing_public: identity.signing_public.clone(),
        owner_fingerprint: identity.owner_fingerprint.clone(),
        signature: String::new(),
    };
    let bytes = invitation_bytes(&invitation)?;
    invitation.signature = sign(&identity.signing_secret, &bytes)?;
    if !invitation.valid() || !verify_invitation(&invitation) {
        return Err(Error::InvalidRequest);
    }
    Ok(invitation)
}

pub fn invitation_fingerprint(invitation: &PersonalSyncInvitation) -> Result<String, Error> {
    if !invitation.valid() {
        return Err(Error::InvalidRequest);
    }
    let digest = Sha256::digest(invitation_bytes(invitation)?);
    Ok(hex(&digest[..16]))
}

pub fn verify_invitation(invitation: &PersonalSyncInvitation) -> bool {
    if !invitation.valid()
        || owner_fingerprint(&invitation.owner_id).as_deref() != Ok(&invitation.owner_fingerprint)
        || AgeRecipient::from_str(&invitation.age_recipient).is_err()
    {
        return false;
    }
    invitation_bytes(invitation)
        .is_ok_and(|bytes| verify(&invitation.signing_public, &bytes, &invitation.signature))
}

fn encrypt_with<R: age::Recipient>(recipient: &R, plaintext: &[u8]) -> Result<String, Error> {
    let encryptor = age::Encryptor::with_recipients(std::iter::once(recipient as _))
        .map_err(|_| Error::Encryption)?;
    let mut ciphertext = Vec::new();
    let armor = ArmoredWriter::wrap_output(&mut ciphertext, Format::AsciiArmor)
        .map_err(|_| Error::Encryption)?;
    let mut writer = encryptor
        .wrap_output(armor)
        .map_err(|_| Error::Encryption)?;
    writer
        .write_all(plaintext)
        .and_then(|_| writer.finish())
        .and_then(|armor| armor.finish())
        .map_err(|_| Error::Encryption)?;
    String::from_utf8(ciphertext).map_err(|_| Error::Encryption)
}

fn valid_passphrase(value: &str) -> bool {
    value.chars().count() >= 12 && value.len() <= 1024
}

fn passphrase_recipient(value: &str) -> Result<age::scrypt::Recipient, Error> {
    if !valid_passphrase(value) {
        return Err(Error::InvalidRequest);
    }
    let mut recipient = age::scrypt::Recipient::new(value.to_owned().into());
    recipient.set_work_factor(16);
    Ok(recipient)
}

pub fn seal_bundle(
    bundle: &PersonalSyncBundle,
    sender_device_id: &str,
    signing_secret: &str,
    recipient: Option<&str>,
    passphrase: Option<&str>,
) -> Result<PersonalSyncEnvelope, Error> {
    if !bundle.valid()
        || recipient.is_some() == passphrase.is_some()
        || !precept_core::operator::valid_identity(sender_device_id)
        || bundle
            .entries
            .iter()
            .any(|entry| !entry.clock.contains_key(sender_device_id))
        || bundle
            .tombstones
            .iter()
            .any(|tombstone| !tombstone.clock.contains_key(sender_device_id))
    {
        return Err(Error::InvalidRequest);
    }
    let plaintext = serde_json::to_vec(bundle).map_err(|_| Error::InvalidRequest)?;
    if plaintext.len() > MAX_SYNC_PAYLOAD_BYTES {
        return Err(Error::TooLarge);
    }
    let ciphertext = match (recipient, passphrase) {
        (Some(recipient), None) => {
            let recipient = AgeRecipient::from_str(recipient).map_err(|_| Error::InvalidRequest)?;
            encrypt_with(&recipient, &plaintext)?
        }
        (None, Some(passphrase)) => encrypt_with(&passphrase_recipient(passphrase)?, &plaintext)?,
        _ => return Err(Error::InvalidRequest),
    };
    if ciphertext.len() > MAX_SYNC_ENVELOPE_BYTES {
        return Err(Error::TooLarge);
    }
    let header = PersonalSyncHeader {
        format: PERSONAL_SYNC_FORMAT.into(),
        version: PERSONAL_SYNC_VERSION,
        bundle_id: bundle.bundle_id.clone(),
        sender_device_id: sender_device_id.to_owned(),
        signing_public: hex(signing_key(signing_secret)?.verifying_key().as_bytes()),
        owner_fingerprint: owner_fingerprint(&bundle.owner_id)?,
    };
    let bytes = signature_bytes(&header, &ciphertext)?;
    let signature = sign(signing_secret, &bytes)?;
    let envelope = PersonalSyncEnvelope {
        format: PERSONAL_SYNC_FORMAT.into(),
        version: PERSONAL_SYNC_VERSION,
        header,
        ciphertext,
        signature,
    };
    if !envelope.valid() {
        return Err(Error::InvalidRequest);
    }
    Ok(envelope)
}

fn decrypt_with<I: age::Identity>(identity: &I, ciphertext: &str) -> Result<Vec<u8>, Error> {
    let decryptor = age::Decryptor::new(ArmoredReader::new(Cursor::new(ciphertext.as_bytes())))
        .map_err(|_| Error::Decryption)?;
    let reader = decryptor
        .decrypt(std::iter::once(identity as _))
        .map_err(|_| Error::Decryption)?;
    let mut plaintext = Vec::new();
    reader
        .take(MAX_SYNC_PAYLOAD_BYTES as u64 + 1)
        .read_to_end(&mut plaintext)
        .map_err(|_| Error::Decryption)?;
    if plaintext.len() > MAX_SYNC_PAYLOAD_BYTES {
        return Err(Error::TooLarge);
    }
    Ok(plaintext)
}

pub fn open_bundle(
    envelope: &PersonalSyncEnvelope,
    identity_secret: Option<&str>,
    passphrase: Option<&str>,
    trusted: &TrustedSender,
) -> Result<PersonalSyncBundle, Error> {
    if !envelope.valid()
        || identity_secret.is_some() == passphrase.is_some()
        || !precept_core::operator::valid_identity(&trusted.owner_id)
        || !precept_core::operator::valid_identity(&trusted.device_id)
        || envelope.header.sender_device_id != trusted.device_id
        || envelope.header.signing_public != trusted.signing_public
        || owner_fingerprint(&trusted.owner_id)? != envelope.header.owner_fingerprint
    {
        return Err(Error::Authentication);
    }
    let signed = signature_bytes(&envelope.header, &envelope.ciphertext)?;
    if !verify(&trusted.signing_public, &signed, &envelope.signature) {
        return Err(Error::Authentication);
    }
    let plaintext = match (identity_secret, passphrase) {
        (Some(identity), None) => {
            let identity = AgeIdentity::from_str(identity).map_err(|_| Error::InvalidRequest)?;
            decrypt_with(&identity, &envelope.ciphertext)?
        }
        (None, Some(passphrase)) => {
            if !valid_passphrase(passphrase) {
                return Err(Error::InvalidRequest);
            }
            let mut identity = age::scrypt::Identity::new(passphrase.to_owned().into());
            identity.set_max_work_factor(16);
            decrypt_with(&identity, &envelope.ciphertext)?
        }
        _ => return Err(Error::InvalidRequest),
    };
    let bundle: PersonalSyncBundle =
        serde_json::from_slice(&plaintext).map_err(|_| Error::InvalidRequest)?;
    if !bundle.valid()
        || bundle.bundle_id != envelope.header.bundle_id
        || bundle.owner_id != trusted.owner_id
        || bundle
            .entries
            .iter()
            .any(|entry| !entry.clock.contains_key(&trusted.device_id))
        || bundle
            .tombstones
            .iter()
            .any(|tombstone| !tombstone.clock.contains_key(&trusted.device_id))
    {
        return Err(Error::InvalidRequest);
    }
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use precept_core::operator::{PersonalSyncContent, PersonalSyncEntry, RuleOptions, clock_tick};
    use std::collections::BTreeMap;

    fn bundle(owner: &str, device: &str) -> PersonalSyncBundle {
        let mut clock = BTreeMap::new();
        clock.insert(device.to_owned(), 1);
        let content = PersonalSyncContent {
            key: "tone".into(),
            value: "Short answers.".into(),
            tags: vec!["style".into()],
            rule: RuleOptions::default(),
            enabled: true,
        };
        let digest = content.digest();
        PersonalSyncBundle {
            format: PERSONAL_SYNC_FORMAT.into(),
            version: PERSONAL_SYNC_VERSION,
            bundle_id: "b".repeat(32),
            owner_id: owner.into(),
            created_at_ms: 1,
            entries: vec![PersonalSyncEntry {
                content,
                clock,
                digest,
            }],
            tombstones: vec![],
        }
    }

    fn trusted(sender: &GeneratedIdentity) -> TrustedSender {
        TrustedSender {
            owner_id: sender.owner_id.clone(),
            device_id: sender.device_id.clone(),
            signing_public: sender.signing_public.clone(),
        }
    }

    #[test]
    fn encrypted_bundles_authenticate_sender_owner_and_recipient() {
        let sender = generate_identity().unwrap();
        let recipient = generate_identity().unwrap();
        let bundle = bundle(&sender.owner_id, &sender.device_id);
        let envelope = seal_bundle(
            &bundle,
            &sender.device_id,
            &sender.signing_secret,
            Some(&recipient.age_recipient),
            None,
        )
        .unwrap();
        assert_eq!(
            open_bundle(
                &envelope,
                Some(&recipient.age_identity),
                None,
                &trusted(&sender)
            )
            .unwrap(),
            bundle
        );
        assert!(
            open_bundle(
                &envelope,
                Some(&sender.age_identity),
                None,
                &trusted(&sender)
            )
            .is_err()
        );
        let mut tampered = envelope.clone();
        tampered.ciphertext.push('x');
        assert!(
            open_bundle(
                &tampered,
                Some(&recipient.age_identity),
                None,
                &trusted(&sender)
            )
            .is_err()
        );
        let attacker = generate_identity().unwrap();
        assert!(
            open_bundle(
                &envelope,
                Some(&recipient.age_identity),
                None,
                &trusted(&attacker)
            )
            .is_err()
        );
        let mut wrong_owner = bundle;
        wrong_owner.owner_id = "c".repeat(32);
        let envelope = seal_bundle(
            &wrong_owner,
            &sender.device_id,
            &sender.signing_secret,
            Some(&recipient.age_recipient),
            None,
        )
        .unwrap();
        assert!(
            open_bundle(
                &envelope,
                Some(&recipient.age_identity),
                None,
                &trusted(&sender)
            )
            .is_err()
        );
    }

    #[test]
    fn passphrase_mode_still_requires_a_valid_device_signature() {
        let sender = generate_identity().unwrap();
        let bundle = bundle(&sender.owner_id, &sender.device_id);
        let envelope = seal_bundle(
            &bundle,
            &sender.device_id,
            &sender.signing_secret,
            None,
            Some("correct horse battery staple"),
        )
        .unwrap();
        assert_eq!(
            open_bundle(
                &envelope,
                None,
                Some("correct horse battery staple"),
                &trusted(&sender)
            )
            .unwrap(),
            bundle
        );
        assert!(open_bundle(&envelope, None, Some("wrong passphrase"), &trusted(&sender)).is_err());
        assert!(
            seal_bundle(
                &bundle,
                &sender.device_id,
                &sender.signing_secret,
                None,
                Some("short"),
            )
            .is_err()
        );
    }

    #[test]
    fn invitation_fingerprints_bind_the_complete_device_keys() {
        let first = generate_identity().unwrap();
        let second = generate_identity_for_owner(&first.owner_id).unwrap();
        let first_invitation = invitation(&first).unwrap();
        let second_invitation = invitation(&second).unwrap();
        assert_eq!(first.owner_fingerprint, second.owner_fingerprint);
        assert_ne!(
            invitation_fingerprint(&first_invitation).unwrap(),
            invitation_fingerprint(&second_invitation).unwrap()
        );
    }

    #[test]
    fn key_rotation_preserves_owner_and_device_identity() {
        let before = generate_identity().unwrap();
        let after = rotate_identity(&before.owner_id, &before.device_id).unwrap();
        assert_eq!(after.owner_id, before.owner_id);
        assert_eq!(after.device_id, before.device_id);
        assert_eq!(after.owner_fingerprint, before.owner_fingerprint);
        assert_ne!(after.age_identity, before.age_identity);
        assert_ne!(after.signing_secret, before.signing_secret);
        assert!(verify_invitation(&invitation(&after).unwrap()));
    }

    #[test]
    fn invitations_are_self_consistent_but_trust_remains_explicit() {
        let identity = generate_identity().unwrap();
        let invitation = invitation(&identity).unwrap();
        assert!(verify_invitation(&invitation));
        let mut changed = invitation.clone();
        changed.age_recipient = identity.age_recipient.replace("age1", "age2");
        assert!(!verify_invitation(&changed));
        assert_eq!(
            clock_tick(&BTreeMap::new(), &identity.device_id).unwrap()[&identity.device_id],
            1
        );
        let debug = format!("{identity:?}");
        assert!(!debug.contains(&identity.age_identity));
        assert!(!debug.contains(&identity.signing_secret));
        assert!(sign(&"é".repeat(32), b"synthetic").is_err());
        assert!(!verify(&"é".repeat(64), b"synthetic", &"0".repeat(128)));
    }
}
