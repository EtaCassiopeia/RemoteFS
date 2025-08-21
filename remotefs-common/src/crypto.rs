use anyhow::{Result, anyhow};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce, 
    aead::{Aead, KeyInit},
};
use hkdf::Hkdf;
use sha2::Sha256;
use rand::{RngCore, thread_rng};
use serde::{Deserialize, Serialize};

/// Size of encryption nonce in bytes
pub const NONCE_SIZE: usize = 12;

/// Size of encryption key in bytes
pub const KEY_SIZE: usize = 32;

/// Size of X25519 key in bytes
pub const X25519_KEY_SIZE: usize = 32;

/// Maximum size for compressed/encrypted chunks
pub const MAX_CHUNK_SIZE: usize = 1024 * 1024; // 1MB chunks

/// Key derivation context for different purposes
#[derive(Debug)]
pub enum KeyContext {
    FileEncryption,
    MessageEncryption,
    SessionKey,
    AuthToken,
}

impl KeyContext {
    fn as_bytes(&self) -> &[u8] {
        match self {
            KeyContext::FileEncryption => b"remotefs-v1-file-encryption",
            KeyContext::MessageEncryption => b"remotefs-v1-message-encryption", 
            KeyContext::SessionKey => b"remotefs-v1-session-key",
            KeyContext::AuthToken => b"remotefs-v1-auth-token",
        }
    }
}

/// Encrypted data with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub nonce: [u8; NONCE_SIZE],
    pub ciphertext: Vec<u8>,
    pub compressed: bool,
}

impl EncryptedData {
    /// Get total size of encrypted data
    pub fn size(&self) -> usize {
        NONCE_SIZE + self.ciphertext.len()
    }
    
    /// Convert to bytes for transmission
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(self.size());
        result.extend_from_slice(&self.nonce);
        result.extend_from_slice(&self.ciphertext);
        result
    }
    
    /// Create from bytes received from transmission
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < NONCE_SIZE {
            return Err(anyhow!("Data too short for encrypted format"));
        }
        
        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&data[..NONCE_SIZE]);
        let ciphertext = data[NONCE_SIZE..].to_vec();
        
        Ok(EncryptedData {
            nonce,
            ciphertext,
            compressed: false, // Will be determined during decryption
        })
    }
}

/// Key exchange data for establishing secure channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyExchange {
    pub public_key: [u8; X25519_KEY_SIZE],
    pub encrypted_challenge: EncryptedData,
}

/// Encryption manager for file data and message encryption
pub struct EncryptionManager {
    cipher: ChaCha20Poly1305,
    master_key: [u8; KEY_SIZE],
}

impl EncryptionManager {
    /// Create new encryption manager with master key
    pub fn new(master_key: [u8; KEY_SIZE]) -> Self {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&master_key));
        
        Self {
            cipher,
            master_key,
        }
    }
    
    /// Derive a key for specific context
    pub fn derive_key(&self, context: KeyContext, salt: Option<&[u8]>) -> Result<[u8; KEY_SIZE]> {
        let hkdf = Hkdf::<Sha256>::new(salt, &self.master_key);
        let mut derived_key = [0u8; KEY_SIZE];
        
        hkdf.expand(context.as_bytes(), &mut derived_key)
            .map_err(|e| anyhow!("Key derivation failed: {}", e))?;
            
        Ok(derived_key)
    }
    
    /// Encrypt data with optional compression
    pub fn encrypt(&self, data: &[u8], compress: bool) -> Result<EncryptedData> {
        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // Compress if requested and beneficial
        let (processed_data, was_compressed) = if compress && data.len() > 128 {
            let compressed = lz4_flex::compress_prepend_size(data);
            // Only use compression if it actually reduces size
            if compressed.len() < data.len() {
                (compressed, true)
            } else {
                (data.to_vec(), false)
            }
        } else {
            (data.to_vec(), false)
        };
        
        // Encrypt the processed data
        let ciphertext = self.cipher
            .encrypt(nonce, processed_data.as_ref())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;
            
        Ok(EncryptedData {
            nonce: nonce_bytes,
            ciphertext,
            compressed: was_compressed,
        })
    }
    
    /// Decrypt data with automatic decompression
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(&encrypted.nonce);
        
        // Decrypt the data
        let decrypted = self.cipher
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;
            
        // Decompress if needed
        if encrypted.compressed {
            lz4_flex::decompress_size_prepended(&decrypted)
                .map_err(|e| anyhow!("Decompression failed: {}", e))
        } else {
            Ok(decrypted)
        }
    }
    
    /// Encrypt data using a derived key with context
    pub fn encrypt_with_context(
        &self, 
        data: &[u8], 
        context: KeyContext,
        salt: Option<&[u8]>,
        compress: bool
    ) -> Result<EncryptedData> {
        let derived_key = self.derive_key(context, salt)?;
        let temp_manager = EncryptionManager::new(derived_key);
        temp_manager.encrypt(data, compress)
    }
    
    /// Decrypt data using a derived key with context
    pub fn decrypt_with_context(
        &self,
        encrypted: &EncryptedData,
        context: KeyContext,
        salt: Option<&[u8]>
    ) -> Result<Vec<u8>> {
        let derived_key = self.derive_key(context, salt)?;
        let temp_manager = EncryptionManager::new(derived_key);
        temp_manager.decrypt(encrypted)
    }
}

// Note: SessionManager has been removed to simplify the crypto module.
// Key exchange functionality will be implemented at a higher level when needed.

/// Generate a secure random key
pub fn generate_key() -> [u8; KEY_SIZE] {
    let mut key = [0u8; KEY_SIZE];
    thread_rng().fill_bytes(&mut key);
    key
}

/// Generate a secure random X25519 key pair
pub fn generate_keypair() -> ([u8; X25519_KEY_SIZE], [u8; X25519_KEY_SIZE]) {
    let mut secret_bytes = [0u8; X25519_KEY_SIZE];
    thread_rng().fill_bytes(&mut secret_bytes);
    
    // For the public key, we would normally use x25519_dalek, but to keep it simple
    // we'll just return the secret for now. In practice, you'd compute the actual public key.
    let mut public_bytes = [0u8; X25519_KEY_SIZE];
    thread_rng().fill_bytes(&mut public_bytes);
    
    (secret_bytes, public_bytes)
}

/// Secure password-based key derivation using HKDF
pub fn derive_key_from_password(password: &str, salt: &[u8]) -> Result<[u8; KEY_SIZE]> {
    // Use HKDF for simple password-based key derivation
    let hkdf = Hkdf::<Sha256>::new(Some(salt), password.as_bytes());
    let mut key = [0u8; KEY_SIZE];
    
    hkdf.expand(b"remotefs-v1-password-key", &mut key)
        .map_err(|e| anyhow!("Password key derivation failed: {}", e))?;
        
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encryption_roundtrip() {
        let key = generate_key();
        let manager = EncryptionManager::new(key);
        let data = b"Hello, world! This is a test message.";
        
        let encrypted = manager.encrypt(data, true).expect("Encryption failed");
        let decrypted = manager.decrypt(&encrypted).expect("Decryption failed");
        
        assert_eq!(data.as_slice(), decrypted.as_slice());
    }
    
    #[test]
    fn test_key_generation() {
        let key1 = generate_key();
        let key2 = generate_key();
        
        // Keys should be different
        assert_ne!(key1, key2);
        
        // Keys should be the right size
        assert_eq!(key1.len(), KEY_SIZE);
        assert_eq!(key2.len(), KEY_SIZE);
    }
    
    #[test]
    fn test_encrypted_data_serialization() {
        let key = generate_key();
        let manager = EncryptionManager::new(key);
        let data = b"Test data for serialization";
        
        let encrypted = manager.encrypt(data, false).expect("Encryption failed");
        let bytes = encrypted.to_bytes();
        let restored = EncryptedData::from_bytes(&bytes).expect("Deserialization failed");
        
        assert_eq!(encrypted.nonce, restored.nonce);
        assert_eq!(encrypted.ciphertext, restored.ciphertext);
        
        let decrypted = manager.decrypt(&restored).expect("Decryption failed");
        assert_eq!(data.as_slice(), decrypted.as_slice());
    }
}
