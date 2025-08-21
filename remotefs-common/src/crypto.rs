use anyhow::{Result, anyhow};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce, 
    aead::{Aead, KeyInit, generic_array::GenericArray},
};
use hkdf::Hkdf;
use sha2::Sha256;
use rand::{RngCore, thread_rng};
use x25519_dalek::{PublicKey, EphemeralSecret, StaticSecret};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

/// Session encryption manager for client-agent communication
pub struct SessionManager {
    secret_key: StaticSecret,
    public_key: PublicKey,
    shared_secrets: dashmap::DashMap<String, Arc<EncryptionManager>>,
}

impl SessionManager {
    /// Create new session manager with random key pair
    pub fn new() -> Self {
        let secret_key = StaticSecret::random_from_rng(&mut thread_rng());
        let public_key = PublicKey::from(&secret_key);
        
        Self {
            secret_key,
            public_key,
            shared_secrets: dashmap::DashMap::new(),
        }
    }
    
    /// Create session manager from existing secret key
    pub fn from_secret(secret_bytes: [u8; X25519_KEY_SIZE]) -> Self {
        let secret_key = StaticSecret::from(secret_bytes);
        let public_key = PublicKey::from(&secret_key);
        
        Self {
            secret_key,
            public_key,
            shared_secrets: dashmap::DashMap::new(),
        }
    }
    
    /// Get our public key for key exchange
    pub fn public_key(&self) -> [u8; X25519_KEY_SIZE] {
        self.public_key.to_bytes()
    }
    
    /// Create key exchange data for establishing session with remote party
    pub fn create_key_exchange(&self, session_id: &str) -> Result<KeyExchange> {
        // Create a challenge that the other party must encrypt back to us
        let challenge = format!("remotefs-challenge-{}-{}", session_id, uuid::Uuid::new_v4());
        
        // For key exchange, we use a temporary key derived from our secret
        let temp_key = self.derive_temp_key(session_id.as_bytes())?;
        let temp_manager = EncryptionManager::new(temp_key);
        
        let encrypted_challenge = temp_manager.encrypt(challenge.as_bytes(), false)?;
        
        Ok(KeyExchange {
            public_key: self.public_key.to_bytes(),
            encrypted_challenge,
        })
    }
    
    /// Process key exchange from remote party and establish shared secret
    pub fn process_key_exchange(
        &self,
        session_id: &str,
        remote_exchange: &KeyExchange
    ) -> Result<EncryptedData> {
        // Derive shared secret from remote public key
        let remote_public = PublicKey::from(remote_exchange.public_key);
        let shared_secret = self.secret_key.diffie_hellman(&remote_public);
        
        // Create encryption manager from shared secret
        let encryption_key = self.derive_session_key(shared_secret.as_bytes())?;
        let manager = Arc::new(EncryptionManager::new(encryption_key));
        
        // Store the shared encryption manager
        self.shared_secrets.insert(session_id.to_string(), manager.clone());
        
        // Decrypt their challenge using temporary key
        let temp_key = self.derive_temp_key(session_id.as_bytes())?;
        let temp_manager = EncryptionManager::new(temp_key);
        let challenge = temp_manager.decrypt(&remote_exchange.encrypted_challenge)?;
        
        // Encrypt the challenge back using our shared session key
        manager.encrypt(&challenge, false)
    }
    
    /// Complete key exchange by verifying the returned challenge
    pub fn complete_key_exchange(
        &self,
        session_id: &str,
        remote_public_key: [u8; X25519_KEY_SIZE],
        encrypted_response: &EncryptedData
    ) -> Result<()> {
        // Derive shared secret
        let remote_public = PublicKey::from(remote_public_key);
        let shared_secret = self.secret_key.diffie_hellman(&remote_public);
        
        // Create encryption manager from shared secret
        let encryption_key = self.derive_session_key(shared_secret.as_bytes())?;
        let manager = Arc::new(EncryptionManager::new(encryption_key));
        
        // Decrypt their response
        let decrypted_challenge = manager.decrypt(encrypted_response)?;
        
        // Verify it matches our original challenge format
        let challenge_str = String::from_utf8(decrypted_challenge)?;
        if !challenge_str.starts_with(&format!("remotefs-challenge-{}", session_id)) {
            return Err(anyhow!("Key exchange verification failed: invalid challenge"));
        }
        
        // Store the verified session
        self.shared_secrets.insert(session_id.to_string(), manager);
        
        Ok(())
    }
    
    /// Get encryption manager for a session
    pub fn get_session(&self, session_id: &str) -> Option<Arc<EncryptionManager>> {
        self.shared_secrets.get(session_id).map(|v| v.clone())
    }
    
    /// Remove a session
    pub fn remove_session(&self, session_id: &str) {
        self.shared_secrets.remove(session_id);
    }
    
    /// Encrypt data for a specific session
    pub fn encrypt_for_session(
        &self,
        session_id: &str,
        data: &[u8],
        compress: bool
    ) -> Result<EncryptedData> {
        let manager = self.get_session(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;
            
        manager.encrypt(data, compress)
    }
    
    /// Decrypt data from a specific session  
    pub fn decrypt_from_session(
        &self,
        session_id: &str,
        encrypted: &EncryptedData
    ) -> Result<Vec<u8>> {
        let manager = self.get_session(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;
            
        manager.decrypt(encrypted)
    }
    
    fn derive_temp_key(&self, salt: &[u8]) -> Result<[u8; KEY_SIZE]> {
        let hkdf = Hkdf::<Sha256>::new(Some(salt), self.secret_key.as_bytes());
        let mut key = [0u8; KEY_SIZE];
        hkdf.expand(b"remotefs-v1-temp-key", &mut key)
            .map_err(|e| anyhow!("Temp key derivation failed: {}", e))?;
        Ok(key)
    }
    
    fn derive_session_key(&self, shared_secret: &[u8]) -> Result<[u8; KEY_SIZE]> {
        let hkdf = Hkdf::<Sha256>::new(None, shared_secret);
        let mut key = [0u8; KEY_SIZE];
        hkdf.expand(b"remotefs-v1-session-key", &mut key)
            .map_err(|e| anyhow!("Session key derivation failed: {}", e))?;
        Ok(key)
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a secure random key
pub fn generate_key() -> [u8; KEY_SIZE] {
    let mut key = [0u8; KEY_SIZE];
    thread_rng().fill_bytes(&mut key);
    key
}

/// Generate a secure random X25519 key pair
pub fn generate_keypair() -> (StaticSecret, PublicKey) {
    let secret = StaticSecret::random_from_rng(&mut thread_rng());
    let public = PublicKey::from(&secret);
    (secret, public)
}

/// Secure password-based key derivation using Argon2id
pub fn derive_key_from_password(password: &str, salt: &[u8]) -> Result<[u8; KEY_SIZE]> {
    use argon2::{Argon2, PasswordHasher, PasswordHash};
    
    // Use strong Argon2id parameters
    let argon2 = Argon2::default();
    
    // Create a password hash string (we only need the key derivation)
    let password_hash = argon2
        .hash_password(password.as_bytes(), salt)
        .map_err(|e| anyhow!("Password hashing failed: {}", e))?;
        
    let parsed_hash = PasswordHash::new(&password_hash.to_string())
        .map_err(|e| anyhow!("Password hash parsing failed: {}", e))?;
        
    // Extract the key from the hash
    let key_bytes = parsed_hash.hash
        .ok_or_else(|| anyhow!("No hash in password hash"))?
        .as_bytes();
        
    if key_bytes.len() < KEY_SIZE {
        return Err(anyhow!("Derived key too short"));
    }
    
    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(&key_bytes[..KEY_SIZE]);
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
    fn test_key_exchange() {
        let client_session = SessionManager::new();
        let agent_session = SessionManager::new();
        let session_id = "test-session";
        
        // Client initiates key exchange
        let client_exchange = client_session
            .create_key_exchange(session_id)
            .expect("Client key exchange failed");
            
        // Agent processes key exchange and responds
        let agent_response = agent_session
            .process_key_exchange(session_id, &client_exchange)
            .expect("Agent key exchange failed");
            
        // Client completes key exchange
        client_session
            .complete_key_exchange(session_id, agent_session.public_key(), &agent_response)
            .expect("Key exchange completion failed");
            
        // Test encryption between sessions
        let test_data = b"This is a test message for session encryption";
        
        let encrypted = client_session
            .encrypt_for_session(session_id, test_data, false)
            .expect("Session encryption failed");
            
        let decrypted = agent_session
            .decrypt_from_session(session_id, &encrypted)
            .expect("Session decryption failed");
            
        assert_eq!(test_data.as_slice(), decrypted.as_slice());
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
