use std::error::Error;
use std::fmt::{Display, Formatter};

use argon2::{
    password_hash::{
        rand_core::OsRng, Error as PasswordHashLibraryError, PasswordHash, PasswordHasher,
        PasswordVerifier, SaltString,
    },
    Algorithm, Argon2, Params, Version,
};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

const LEGACY_SHA256_HEX_LENGTH: usize = 64;
pub(crate) const CANONICAL_ARGON2_VERIFIER_STORAGE_LENGTH: usize = 97;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PasswordHashError {
    Hashing(String),
    InvalidVerifier(String),
}

impl Display for PasswordHashError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hashing(error) => write!(formatter, "password hash failed: {error}"),
            Self::InvalidVerifier(error) => write!(formatter, "invalid password hash: {error}"),
        }
    }
}

impl Error for PasswordHashError {}

pub(crate) fn hash_password(password: &str) -> Result<String, PasswordHashError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| PasswordHashError::Hashing(error.to_string()))
}

pub(crate) fn verify_password(
    password: &str,
    password_verifier: &str,
) -> Result<bool, PasswordHashError> {
    match PasswordHash::new(password_verifier) {
        Ok(parsed) => {
            validate_argon2_verifier_contract(&parsed)?;
            match Argon2::default().verify_password(password.as_bytes(), &parsed) {
                Ok(()) => Ok(true),
                Err(PasswordHashLibraryError::Password) => Ok(false),
                Err(verification_error) => Err(PasswordHashError::InvalidVerifier(
                    verification_error.to_string(),
                )),
            }
        }
        Err(_) if is_legacy_sha256(password_verifier) => {
            Ok(verify_legacy_sha256(password, password_verifier))
        }
        Err(parse_error) => Err(PasswordHashError::InvalidVerifier(parse_error.to_string())),
    }
}

fn validate_argon2_verifier_contract(
    password_hash: &PasswordHash<'_>,
) -> Result<(), PasswordHashError> {
    if password_hash.algorithm != Algorithm::Argon2id.ident() {
        return Err(PasswordHashError::InvalidVerifier(
            "unsupported algorithm".to_string(),
        ));
    }

    if password_hash.version != Some(u32::from(Version::V0x13)) {
        return Err(PasswordHashError::InvalidVerifier(
            "invalid algorithm version".to_string(),
        ));
    }

    let params = Params::try_from(password_hash)
        .map_err(|error| PasswordHashError::InvalidVerifier(error.to_string()))?;
    if params.m_cost() != Params::DEFAULT_M_COST
        || params.t_cost() != Params::DEFAULT_T_COST
        || params.p_cost() != Params::DEFAULT_P_COST
    {
        return Err(PasswordHashError::InvalidVerifier(format!(
            "unsupported Argon2 parameters: m={},t={},p={}",
            params.m_cost(),
            params.t_cost(),
            params.p_cost()
        )));
    }

    if password_hash.hash.as_ref().map(|output| output.len()) != Some(Params::DEFAULT_OUTPUT_LEN) {
        return Err(PasswordHashError::InvalidVerifier(
            "unsupported Argon2 output length".to_string(),
        ));
    }

    Ok(())
}

pub(crate) fn is_legacy_sha256(password_verifier: &str) -> bool {
    password_verifier.len() == LEGACY_SHA256_HEX_LENGTH
        && password_verifier
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
fn hash_legacy_sha256(password: &str) -> String {
    hex::encode(Sha256::digest(password.as_bytes()))
}

fn verify_legacy_sha256(password: &str, password_verifier: &str) -> bool {
    let mut expected_digest = [0_u8; 32];
    if hex::decode_to_slice(password_verifier, &mut expected_digest).is_err() {
        return false;
    }

    let actual_digest = Sha256::digest(password.as_bytes());
    bool::from(actual_digest[..].ct_eq(&expected_digest))
}

#[cfg(test)]
mod tests {
    use super::{
        hash_legacy_sha256, hash_password, is_legacy_sha256, verify_password, PasswordHashError,
        CANONICAL_ARGON2_VERIFIER_STORAGE_LENGTH, LEGACY_SHA256_HEX_LENGTH,
    };

    #[test]
    fn argon2_password_hash_is_salted_phc_and_exceeds_legacy_column_width() {
        let first = hash_password("admin123").expect("first Argon2 hash should succeed");
        let second = hash_password("admin123").expect("second Argon2 hash should succeed");

        assert!(first.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert!(second.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert_ne!(first, second, "Argon2 hashes must use independent salts");
        assert_eq!(
            first.len(),
            CANONICAL_ARGON2_VERIFIER_STORAGE_LENGTH,
            "readiness storage contract must track the canonical PHC shape"
        );
        assert_eq!(second.len(), CANONICAL_ARGON2_VERIFIER_STORAGE_LENGTH);
        assert!(
            first.len() > LEGACY_SHA256_HEX_LENGTH,
            "Argon2 PHC verifier must demonstrate why VARCHAR(64) is insufficient"
        );
        assert!(verify_password("admin123", &first).expect("Argon2 verifier should parse"));
        assert!(!verify_password("wrong-password", &first).expect("wrong password is not an error"));
    }

    #[test]
    fn legacy_sha256_verifier_remains_compatible() {
        let legacy = hash_legacy_sha256("admin123");

        assert!(is_legacy_sha256(&legacy));
        assert!(verify_password("admin123", &legacy).expect("legacy SHA-256 should verify"));
        assert!(
            !verify_password("wrong-password", &legacy).expect("wrong password is not an error")
        );
        assert!(verify_password("admin123", &legacy.to_ascii_uppercase())
            .expect("uppercase legacy SHA-256 should remain compatible"));
    }

    #[test]
    fn legacy_sha256_verifier_rejects_boundary_nibble_mismatches() {
        let legacy = hash_legacy_sha256("admin123");
        let mut first_mismatch = legacy.clone().into_bytes();
        first_mismatch[0] = if first_mismatch[0] == b'0' {
            b'1'
        } else {
            b'0'
        };
        let first_mismatch = String::from_utf8(first_mismatch).expect("hex remains UTF-8");

        let mut last_mismatch = legacy.into_bytes();
        let last_index = last_mismatch.len() - 1;
        last_mismatch[last_index] = if last_mismatch[last_index] == b'0' {
            b'1'
        } else {
            b'0'
        };
        let last_mismatch = String::from_utf8(last_mismatch).expect("hex remains UTF-8");

        assert!(!verify_password("admin123", &first_mismatch)
            .expect("first-nibble mismatch is a normal password mismatch"));
        assert!(!verify_password("admin123", &last_mismatch)
            .expect("last-nibble mismatch is a normal password mismatch"));
    }

    #[test]
    fn malformed_password_verifier_returns_explicit_error() {
        let error = verify_password("admin123", "not-a-password-verifier")
            .expect_err("malformed verifier must not be treated as a password mismatch");

        assert!(error.to_string().starts_with("invalid password hash: "));
    }

    #[test]
    fn unsupported_phc_algorithm_returns_explicit_error() {
        let verifier = hash_password("admin123").expect("Argon2 hash should succeed");
        let unsupported = verifier.replacen("$argon2id$", "$scrypt$", 1);

        let error = verify_password("admin123", &unsupported)
            .expect_err("unsupported PHC algorithm must not look like a wrong password");

        assert_eq!(
            error,
            PasswordHashError::InvalidVerifier("unsupported algorithm".to_string())
        );
    }

    #[test]
    fn unsupported_phc_version_returns_explicit_error() {
        let verifier = hash_password("admin123").expect("Argon2 hash should succeed");
        let unsupported = verifier.replacen("$v=19$", "$v=42$", 1);
        assert_ne!(unsupported, verifier, "test must replace the PHC version");

        let error = verify_password("admin123", &unsupported)
            .expect_err("unsupported PHC version must not look like a wrong password");

        assert_eq!(
            error,
            PasswordHashError::InvalidVerifier("invalid algorithm version".to_string())
        );
    }

    #[test]
    fn invalid_phc_parameter_returns_explicit_error() {
        let verifier = hash_password("admin123").expect("Argon2 hash should succeed");
        let invalid = verifier.replacen("m=19456", "m=0", 1);
        assert_ne!(invalid, verifier, "test must replace the PHC memory cost");

        let error = verify_password("admin123", &invalid)
            .expect_err("invalid PHC parameters must not look like a wrong password");

        assert!(matches!(error, PasswordHashError::InvalidVerifier(_)));
        assert!(error.to_string().starts_with("invalid password hash: "));
    }

    #[test]
    fn noncanonical_memory_cost_is_rejected_before_password_verification() {
        let verifier = hash_password("admin123").expect("Argon2 hash should succeed");
        let excessive = verifier.replacen("m=19456", "m=65536", 1);
        assert_ne!(excessive, verifier, "test must replace the PHC memory cost");

        let error = verify_password("admin123", &excessive)
            .expect_err("noncanonical memory cost must be rejected");

        assert!(matches!(error, PasswordHashError::InvalidVerifier(_)));
        assert!(error.to_string().contains("unsupported Argon2 parameters"));
    }

    #[test]
    fn noncanonical_time_cost_is_rejected_before_password_verification() {
        let verifier = hash_password("admin123").expect("Argon2 hash should succeed");
        let excessive = verifier.replacen("t=2", "t=3", 1);
        assert_ne!(excessive, verifier, "test must replace the PHC time cost");

        let error = verify_password("admin123", &excessive)
            .expect_err("noncanonical time cost must be rejected");

        assert!(matches!(error, PasswordHashError::InvalidVerifier(_)));
        assert!(error.to_string().contains("unsupported Argon2 parameters"));
    }

    #[test]
    fn noncanonical_parallelism_is_rejected_before_password_verification() {
        let verifier = hash_password("admin123").expect("Argon2 hash should succeed");
        let excessive = verifier.replacen("p=1", "p=2", 1);
        assert_ne!(excessive, verifier, "test must replace the PHC parallelism");

        let error = verify_password("admin123", &excessive)
            .expect_err("noncanonical parallelism must be rejected");

        assert!(matches!(error, PasswordHashError::InvalidVerifier(_)));
        assert!(error.to_string().contains("unsupported Argon2 parameters"));
    }
}
