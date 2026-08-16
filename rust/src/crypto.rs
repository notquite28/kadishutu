use aes::{
    Aes256, Block,
    cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit},
};
use thiserror::Error;

const AES_BLOCK_LENGTH: usize = 16;
const SAVE_KEY: [u8; 32] = *b"0123456789abcdef0123456789abcdef";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CryptoError {
    #[error("AES input length {length} is not divisible by {AES_BLOCK_LENGTH}")]
    InvalidBlockLength { length: usize },
}

pub fn encrypt(bytes: &[u8]) -> Result<Vec<u8>, CryptoError> {
    transform(bytes, &SAVE_KEY, Direction::Encrypt)
}

pub fn decrypt(bytes: &[u8]) -> Result<Vec<u8>, CryptoError> {
    transform(bytes, &SAVE_KEY, Direction::Decrypt)
}

#[derive(Clone, Copy)]
enum Direction {
    Encrypt,
    Decrypt,
}

fn transform(bytes: &[u8], key: &[u8; 32], direction: Direction) -> Result<Vec<u8>, CryptoError> {
    if bytes.len() % AES_BLOCK_LENGTH != 0 {
        return Err(CryptoError::InvalidBlockLength {
            length: bytes.len(),
        });
    }

    let cipher = Aes256::new(key.into());
    let mut output = bytes.to_vec();
    let (blocks, remainder) = Block::slice_as_chunks_mut(&mut output);
    debug_assert!(remainder.is_empty());
    match direction {
        Direction::Encrypt => cipher.encrypt_blocks(blocks),
        Direction::Decrypt => cipher.decrypt_blocks(blocks),
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NIST_KEY: [u8; 32] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d,
        0x1e, 0x1f,
    ];
    const NIST_PLAINTEXT: [u8; 16] = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ];
    const NIST_CIPHERTEXT: [u8; 16] = [
        0x8e, 0xa2, 0xb7, 0xca, 0x51, 0x67, 0x45, 0xbf, 0xea, 0xfc, 0x49, 0x90, 0x4b, 0x49, 0x60,
        0x89,
    ];

    #[test]
    fn aes_256_matches_nist_known_answer() {
        let encrypted = transform(&NIST_PLAINTEXT, &NIST_KEY, Direction::Encrypt).unwrap();
        assert_eq!(encrypted, NIST_CIPHERTEXT);
        let decrypted = transform(&NIST_CIPHERTEXT, &NIST_KEY, Direction::Decrypt).unwrap();
        assert_eq!(decrypted, NIST_PLAINTEXT);
    }

    #[test]
    fn rejects_partial_blocks_without_padding() {
        assert_eq!(
            encrypt(&[0_u8; 15]),
            Err(CryptoError::InvalidBlockLength { length: 15 })
        );
        assert_eq!(
            decrypt(&[0_u8; 17]),
            Err(CryptoError::InvalidBlockLength { length: 17 })
        );
    }

    #[test]
    fn fixed_key_round_trip_is_exact() {
        let plaintext = [0x5a_u8; 32];
        let ciphertext = encrypt(&plaintext).unwrap();
        assert_ne!(ciphertext, plaintext);
        assert_eq!(decrypt(&ciphertext).unwrap(), plaintext);
    }
}
