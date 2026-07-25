//! Builds a CMS `SignedData` (PKCS#7, detached) suitable for embedding in a
//! PAdES/CAdES signature.
//!
//! The raw asymmetric signature is produced on the CIE chip via PKCS#11; this
//! module only assembles the ASN.1 around it. It therefore takes the signer
//! certificate (public, read from the card) plus a `sign` callback that signs
//! the DER-encoded signed attributes on the card.

use der::asn1::{Null, OctetString, SetOfVec, UtcTime};
use der::{Any, Decode, Encode};
use der::oid::ObjectIdentifier;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cie::error::{CieError, CieResult};

// --- OIDs (defined explicitly to avoid const-oid db path churn) ---
const ID_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.1");
const ID_SIGNED_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.2");
const ID_CONTENT_TYPE: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.3");
const ID_MESSAGE_DIGEST: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.4");
const ID_SIGNING_TIME: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.5");
const ID_AA_SIGNING_CERTIFICATE_V2: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.2.47");
const ID_SHA256: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.1");
const RSA_ENCRYPTION: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1");

/// ESSCertIDv2 with the hash algorithm defaulted to SHA-256 (so omitted) and no
/// issuerSerial. Sufficient for a PAdES-B-B signingCertificateV2 attribute.
#[derive(der::Sequence)]
struct EssCertIdV2 {
    cert_hash: OctetString,
}

/// SigningCertificateV2 with a single ESSCertIDv2 and no policies.
#[derive(der::Sequence)]
struct SigningCertificateV2 {
    certs: Vec<EssCertIdV2>,
}

fn any_of<T: Encode>(value: &T) -> CieResult<Any> {
    let der = value.to_der()?;
    Ok(Any::from_der(&der)?)
}

fn algid_sha256() -> spki::AlgorithmIdentifierOwned {
    spki::AlgorithmIdentifier {
        oid: ID_SHA256,
        parameters: None,
    }
}

fn algid_rsa() -> CieResult<spki::AlgorithmIdentifierOwned> {
    Ok(spki::AlgorithmIdentifier {
        oid: RSA_ENCRYPTION,
        parameters: Some(any_of(&Null)?),
    })
}

fn make_attribute(oid: ObjectIdentifier, value: Any) -> CieResult<x509_cert::attr::Attribute> {
    let values = SetOfVec::try_from(vec![value]).map_err(CieError::from)?;
    Ok(x509_cert::attr::Attribute { oid, values })
}

/// Build the set of CMS signed attributes and return both the typed set and its
/// DER encoding as an explicit `SET OF` (tag `0x31`) — the exact bytes that must
/// be signed per RFC 5652.
fn build_signed_attributes(
    cert_der: &[u8],
    content_digest: &[u8],
) -> CieResult<(SetOfVec<x509_cert::attr::Attribute>, Vec<u8>)> {
    // content-type = id-data
    let content_type = make_attribute(ID_CONTENT_TYPE, any_of(&ID_DATA)?)?;

    // message-digest = digest of the signed content
    let md = OctetString::new(content_digest).map_err(CieError::from)?;
    let message_digest = make_attribute(ID_MESSAGE_DIGEST, any_of(&md)?)?;

    // signing-time = now (UTCTime; valid for years 1950-2049)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| CieError::Other(e.to_string()))?;
    let utc = UtcTime::from_unix_duration(now).map_err(CieError::from)?;
    let signing_time = make_attribute(ID_SIGNING_TIME, any_of(&utc)?)?;

    // signing-certificate-v2 (ESS) = SHA-256 of the signer cert
    let cert_hash = sha256(cert_der);
    let scv2 = SigningCertificateV2 {
        certs: vec![EssCertIdV2 {
            cert_hash: OctetString::new(cert_hash.as_slice()).map_err(CieError::from)?,
        }],
    };
    let signing_cert = make_attribute(ID_AA_SIGNING_CERTIFICATE_V2, any_of(&scv2)?)?;

    let set = SetOfVec::try_from(vec![content_type, message_digest, signing_time, signing_cert])
        .map_err(CieError::from)?;

    // The to-be-signed bytes are the SET OF (0x31...) encoding.
    let tbs = set.to_der()?;
    Ok((set, tbs))
}

fn sha256(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

/// Assemble a detached CMS `SignedData` (DER `ContentInfo`).
///
/// * `cert_der` — signer certificate (read from the card).
/// * `content_digest` — SHA-256 of the data being signed (PDF ByteRange, or the
///   whole file for CAdES).
/// * `sign` — callback that signs the given signed-attributes DER on the card
///   with `CKM_SHA256_RSA_PKCS`, returning the raw signature bytes.
pub fn build_detached_pkcs7<F>(
    cert_der: &[u8],
    content_digest: &[u8],
    sign: F,
) -> CieResult<Vec<u8>>
where
    F: FnOnce(&[u8]) -> CieResult<Vec<u8>>,
{
    let cert = x509_cert::Certificate::from_der(cert_der)?;

    let (signed_attrs, tbs) = build_signed_attributes(cert_der, content_digest)?;
    let signature_bytes = sign(&tbs)?;

    let ias = cms::cert::IssuerAndSerialNumber {
        issuer: cert.tbs_certificate.issuer.clone(),
        serial_number: cert.tbs_certificate.serial_number.clone(),
    };

    let signer_info = cms::signed_data::SignerInfo {
        version: cms::content_info::CmsVersion::V1,
        sid: cms::signed_data::SignerIdentifier::IssuerAndSerialNumber(ias),
        digest_alg: algid_sha256(),
        signed_attrs: Some(signed_attrs),
        signature_algorithm: algid_rsa()?,
        signature: cms::signed_data::SignatureValue::new(signature_bytes)
            .map_err(CieError::from)?,
        unsigned_attrs: None,
    };

    let digest_algorithms =
        SetOfVec::try_from(vec![algid_sha256()]).map_err(CieError::from)?;

    let certificates =
        cms::signed_data::CertificateSet::try_from(vec![
            cms::cert::CertificateChoices::Certificate(cert),
        ])
        .map_err(CieError::from)?;

    let signer_infos =
        SetOfVec::try_from(vec![signer_info]).map_err(CieError::from)?;

    let signed_data = cms::signed_data::SignedData {
        version: cms::content_info::CmsVersion::V1,
        digest_algorithms,
        encap_content_info: cms::signed_data::EncapsulatedContentInfo {
            econtent_type: ID_DATA,
            econtent: None,
        },
        certificates: Some(certificates),
        crls: None,
        signer_infos: cms::signed_data::SignerInfos(signer_infos),
    };

    let content_info = cms::content_info::ContentInfo {
        content_type: ID_SIGNED_DATA,
        content: any_of(&signed_data)?,
    };

    Ok(content_info.to_der()?)
}
