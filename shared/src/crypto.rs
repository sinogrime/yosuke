use std::io;

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, consts::U32, generic_array::GenericArray},
};
use rand::TryRngCore;

#[derive(Clone)] // Add Clone trait for the struct
                 // This is because the client clones the Encryption to run asynchronously
pub struct Encryption {
    pub cipher: Aes256Gcm,
    pub _key: GenericArray<u8, U32>,
}

impl Encryption {
    pub fn new(key: &GenericArray<u8, U32>) -> Self { // Key will be passed in as argument
                                                      // because client reads key from config
                                                      // and server generates key passing it through
        let cipher = Aes256Gcm::new(key); // Cipher from AES_GCM library for encrypting/decrypting
        Self { cipher, _key: *key } // Return the newly made struct
    }

    // 'data' is a pointer to an array of bytes, we return either the nonce and encrypted data, or error
    pub fn encrypt(&self, data: &[u8]) -> Result<([u8; 12], Vec<u8>), std::io::Error> {
        let mut nonce_b = [0u8; 12]; // make space for a nonce
        rand::rngs::OsRng
            .try_fill_bytes(&mut nonce_b) // try to fill nonce_b with random bytes
            .map_err(|_e| io::Error::new(io::ErrorKind::Other, "Failed try_fill_bytes()"))?;
        let nonce = Nonce::from_slice(&nonce_b); // convert to struct expected by library
        let encrypted = self
            .cipher
            .encrypt(nonce, data) // finally, encrypt the data with the cipher, using the nonce
            .map_err(|_e| io::Error::new(io::ErrorKind::Other, "Failed encrypt()"))?;
        Ok((nonce_b, encrypted)) // return the nonce and the encrypted data
    }

    // takes nonce and encrypted data, returns either decrypted data as array of bytes, or error
    pub fn decrypt(&self, nonce_b: &[u8; 12], encrypted: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let nonce = Nonce::from_slice(nonce_b); // make nonce struct from bytes, expected by library

        match self.cipher.decrypt(&nonce, encrypted) { // try to decrypt using cipher
            Ok(decrypted) => {
                Ok(decrypted) // return decrypted data
            }
            Err(e) => {
                println!("[!] Decryption failed: {:?}", e);
                Err(io::Error::new(io::ErrorKind::Other, "Failed decrypt()"))
            }
        }
    }
}
