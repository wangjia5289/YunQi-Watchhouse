use std::{
    fs,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{
        Aead, KeyInit, Payload,
        rand_core::{OsRng, RngCore},
    },
};
use tempfile::NamedTempFile;
use zeroize::Zeroizing;

const MAGIC: &[u8; 8] = b"YQWHBKP\0";
const FORMAT_VERSION: u8 = 2;
const SALT_LENGTH: usize = 16;
const NONCE_LENGTH: usize = 24;
const KEY_LENGTH: usize = 32;
const AUTH_TAG_LENGTH: usize = 16;
const CHUNK_SIZE: usize = 1024 * 1024;
const HEADER_LENGTH: usize = MAGIC.len() + 1 + SALT_LENGTH + NONCE_LENGTH + size_of::<u32>();

const ARGON2_MEMORY_KIB: u32 = 19 * 1024;
const ARGON2_ITERATIONS: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub(crate) enum BackupCryptoError {
    #[error("the backup password cannot be empty")]
    EmptyPassword,
    #[error("the encrypted backup header is invalid or unsupported")]
    InvalidFormat,
    #[error("the encrypted backup could not be authenticated")]
    AuthenticationFailed,
    #[error("the backup encryption key could not be derived")]
    KeyDerivationFailed,
    #[error("encrypted backup I/O failed")]
    Io(#[from] std::io::Error),
}

pub(crate) fn encrypt_file(
    source: &Path,
    destination: &Path,
    password: &[u8],
) -> Result<(), BackupCryptoError> {
    validate_password(password)?;

    let source = fs::File::open(source)?;
    let mut reader = BufReader::new(source);
    let mut salt = [0_u8; SALT_LENGTH];
    let mut base_nonce = [0_u8; NONCE_LENGTH];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut base_nonce[..NONCE_LENGTH - size_of::<u64>()]);

    let header = encode_header(&salt, &base_nonce);
    let key = Zeroizing::new(derive_key(password, &salt)?);
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_slice())
        .map_err(|_| BackupCryptoError::KeyDerivationFailed)?;
    let mut temporary = temporary_destination(destination)?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        writer.write_all(&header)?;
        let mut plaintext = Zeroizing::new(vec![0_u8; CHUNK_SIZE]);
        let mut chunk_index = 0_u64;
        loop {
            let length = reader.read(plaintext.as_mut_slice())?;
            if length == 0 {
                break;
            }
            write_encrypted_chunk(
                &mut writer,
                &cipher,
                &header,
                &base_nonce,
                chunk_index,
                &plaintext[..length],
            )?;
            chunk_index = chunk_index
                .checked_add(1)
                .ok_or(BackupCryptoError::InvalidFormat)?;
        }
        write_encrypted_chunk(&mut writer, &cipher, &header, &base_nonce, chunk_index, &[])?;
        writer.flush()?;
    }
    persist_temporary(temporary, destination)
}

pub(crate) fn decrypt_file(
    source: &Path,
    destination: &Path,
    password: &[u8],
) -> Result<(), BackupCryptoError> {
    validate_password(password)?;

    let source = fs::File::open(source)?;
    let mut reader = BufReader::new(source);
    let mut header = [0_u8; HEADER_LENGTH];
    read_format_bytes(&mut reader, &mut header)?;
    if &header[..MAGIC.len()] != MAGIC || header[MAGIC.len()] != FORMAT_VERSION {
        return Err(BackupCryptoError::InvalidFormat);
    }

    let salt_start = MAGIC.len() + 1;
    let nonce_start = salt_start + SALT_LENGTH;
    let salt = &header[salt_start..nonce_start];
    let nonce_end = nonce_start + NONCE_LENGTH;
    let base_nonce: [u8; NONCE_LENGTH] = header[nonce_start..nonce_end]
        .try_into()
        .map_err(|_| BackupCryptoError::InvalidFormat)?;
    let chunk_size = u32::from_le_bytes(
        header[nonce_end..HEADER_LENGTH]
            .try_into()
            .map_err(|_| BackupCryptoError::InvalidFormat)?,
    ) as usize;
    if chunk_size != CHUNK_SIZE {
        return Err(BackupCryptoError::InvalidFormat);
    }
    let key = Zeroizing::new(derive_key(password, salt)?);
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_slice())
        .map_err(|_| BackupCryptoError::KeyDerivationFailed)?;

    let mut temporary = temporary_destination(destination)?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        let mut chunk_index = 0_u64;
        loop {
            let mut encoded_length = [0_u8; size_of::<u32>()];
            read_format_bytes(&mut reader, &mut encoded_length)?;
            let length = u32::from_le_bytes(encoded_length) as usize;
            if length > chunk_size {
                return Err(BackupCryptoError::InvalidFormat);
            }
            let mut ciphertext = vec![0_u8; length + AUTH_TAG_LENGTH];
            read_format_bytes(&mut reader, &mut ciphertext)?;
            let nonce = nonce_for_chunk(&base_nonce, chunk_index);
            let aad = chunk_aad(&header, chunk_index, length as u32);
            let plaintext = Zeroizing::new(
                cipher
                    .decrypt(
                        XNonce::from_slice(&nonce),
                        Payload {
                            msg: &ciphertext,
                            aad: &aad,
                        },
                    )
                    .map_err(|_| BackupCryptoError::AuthenticationFailed)?,
            );
            if length == 0 {
                if !plaintext.is_empty() || reader.read(&mut [0_u8; 1])? != 0 {
                    return Err(BackupCryptoError::InvalidFormat);
                }
                break;
            }
            if plaintext.len() != length {
                return Err(BackupCryptoError::InvalidFormat);
            }
            writer.write_all(plaintext.as_slice())?;
            chunk_index = chunk_index
                .checked_add(1)
                .ok_or(BackupCryptoError::InvalidFormat)?;
        }
        writer.flush()?;
    }
    persist_temporary(temporary, destination)
}

fn validate_password(password: &[u8]) -> Result<(), BackupCryptoError> {
    if password.is_empty() {
        Err(BackupCryptoError::EmptyPassword)
    } else {
        Ok(())
    }
}

fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; KEY_LENGTH], BackupCryptoError> {
    let parameters = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(KEY_LENGTH),
    )
    .map_err(|_| BackupCryptoError::KeyDerivationFailed)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, parameters);
    let mut key = [0_u8; KEY_LENGTH];
    argon2
        .hash_password_into(password, salt, &mut key)
        .map_err(|_| BackupCryptoError::KeyDerivationFailed)?;
    Ok(key)
}

fn encode_header(salt: &[u8; SALT_LENGTH], nonce: &[u8; NONCE_LENGTH]) -> Vec<u8> {
    let mut header = Vec::with_capacity(HEADER_LENGTH);
    header.extend_from_slice(MAGIC);
    header.push(FORMAT_VERSION);
    header.extend_from_slice(salt);
    header.extend_from_slice(nonce);
    header.extend_from_slice(&(CHUNK_SIZE as u32).to_le_bytes());
    header
}

fn write_encrypted_chunk(
    writer: &mut impl Write,
    cipher: &XChaCha20Poly1305,
    header: &[u8],
    base_nonce: &[u8; NONCE_LENGTH],
    chunk_index: u64,
    plaintext: &[u8],
) -> Result<(), BackupCryptoError> {
    let length = u32::try_from(plaintext.len()).map_err(|_| BackupCryptoError::InvalidFormat)?;
    let nonce = nonce_for_chunk(base_nonce, chunk_index);
    let aad = chunk_aad(header, chunk_index, length);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| BackupCryptoError::AuthenticationFailed)?;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(&ciphertext)?;
    Ok(())
}

fn nonce_for_chunk(base_nonce: &[u8; NONCE_LENGTH], chunk_index: u64) -> [u8; NONCE_LENGTH] {
    let mut nonce = *base_nonce;
    nonce[NONCE_LENGTH - size_of::<u64>()..].copy_from_slice(&chunk_index.to_le_bytes());
    nonce
}

fn chunk_aad(header: &[u8], chunk_index: u64, length: u32) -> Vec<u8> {
    let mut aad = Vec::with_capacity(header.len() + size_of::<u64>() + size_of::<u32>());
    aad.extend_from_slice(header);
    aad.extend_from_slice(&chunk_index.to_le_bytes());
    aad.extend_from_slice(&length.to_le_bytes());
    aad
}

fn read_format_bytes(
    reader: &mut impl Read,
    destination: &mut [u8],
) -> Result<(), BackupCryptoError> {
    reader.read_exact(destination).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            BackupCryptoError::InvalidFormat
        } else {
            BackupCryptoError::Io(error)
        }
    })
}

fn temporary_destination(destination: &Path) -> Result<NamedTempFile, BackupCryptoError> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&parent)?;
    NamedTempFile::new_in(parent).map_err(Into::into)
}

fn persist_temporary(
    mut temporary: NamedTempFile,
    destination: &Path,
) -> Result<(), BackupCryptoError> {
    temporary.as_file_mut().sync_all()?;
    temporary
        .persist(destination)
        .map_err(|error| BackupCryptoError::Io(error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_backup_round_trips() {
        let directory = tempfile::tempdir().expect("temp directory");
        let source = directory.path().join("source.sqlite3");
        let encrypted = directory.path().join("backup.yqbackup");
        let restored = directory.path().join("restored.sqlite3");
        let expected = b"SQLite format 3\0test database bytes";
        fs::write(&source, expected).expect("source fixture");

        encrypt_file(&source, &encrypted, b"correct horse battery staple").expect("encrypt");
        decrypt_file(&encrypted, &restored, b"correct horse battery staple").expect("decrypt");

        assert_eq!(fs::read(restored).expect("restored bytes"), expected);
        assert_ne!(fs::read(encrypted).expect("encrypted bytes"), expected);
    }

    #[test]
    fn encrypted_backup_streams_multiple_chunks() {
        let directory = tempfile::tempdir().expect("temp directory");
        let source = directory.path().join("large.sqlite3");
        let encrypted = directory.path().join("large.yqbackup");
        let restored = directory.path().join("large-restored.sqlite3");
        let expected = (0..CHUNK_SIZE + 37)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        fs::write(&source, &expected).expect("large source fixture");

        encrypt_file(&source, &encrypted, b"multi-chunk-password").expect("encrypt");
        decrypt_file(&encrypted, &restored, b"multi-chunk-password").expect("decrypt");

        assert_eq!(fs::read(restored).expect("restored bytes"), expected);
    }

    #[test]
    fn wrong_password_and_tampering_return_the_same_error() {
        let directory = tempfile::tempdir().expect("temp directory");
        let source = directory.path().join("source.sqlite3");
        let encrypted = directory.path().join("backup.yqbackup");
        let restored = directory.path().join("restored.sqlite3");
        fs::write(&source, b"private activity data").expect("source fixture");
        encrypt_file(&source, &encrypted, b"right-password").expect("encrypt");

        let wrong_password = decrypt_file(&encrypted, &restored, b"wrong-password")
            .expect_err("wrong password must fail");
        assert!(matches!(
            wrong_password,
            BackupCryptoError::AuthenticationFailed
        ));

        let mut tampered = fs::read(&encrypted).expect("encrypted bytes");
        let final_byte = tampered.last_mut().expect("authentication tag");
        *final_byte ^= 0x01;
        fs::write(&encrypted, tampered).expect("tampered backup");
        let tampering = decrypt_file(&encrypted, &restored, b"right-password")
            .expect_err("tampering must fail");
        assert!(matches!(tampering, BackupCryptoError::AuthenticationFailed));
    }

    #[test]
    fn invalid_header_is_rejected_before_decryption() {
        let directory = tempfile::tempdir().expect("temp directory");
        let encrypted = directory.path().join("backup.yqbackup");
        let restored = directory.path().join("restored.sqlite3");
        fs::write(&encrypted, b"not an encrypted Watchhouse backup")
            .expect("invalid backup fixture");

        let error =
            decrypt_file(&encrypted, &restored, b"password").expect_err("invalid header must fail");

        assert!(matches!(error, BackupCryptoError::InvalidFormat));
    }

    #[test]
    fn truncation_and_trailing_data_are_rejected_without_replacing_destination() {
        let directory = tempfile::tempdir().expect("temp directory");
        let source = directory.path().join("source.sqlite3");
        let encrypted = directory.path().join("backup.yqbackup");
        let restored = directory.path().join("restored.sqlite3");
        fs::write(&source, b"private activity data").expect("source fixture");
        fs::write(&restored, b"existing destination").expect("destination fixture");
        encrypt_file(&source, &encrypted, b"correct-password").expect("encrypt");

        let original = fs::read(&encrypted).expect("encrypted bytes");
        fs::write(&encrypted, &original[..original.len() - 1]).expect("truncated backup");
        assert!(decrypt_file(&encrypted, &restored, b"correct-password").is_err());
        assert_eq!(fs::read(&restored).unwrap(), b"existing destination");

        let mut trailing = original;
        trailing.push(0x7f);
        fs::write(&encrypted, trailing).expect("backup with trailing data");
        assert!(matches!(
            decrypt_file(&encrypted, &restored, b"correct-password"),
            Err(BackupCryptoError::InvalidFormat)
        ));
        assert_eq!(fs::read(restored).unwrap(), b"existing destination");
    }
}
