use std::path::Path;

use aes_gcm::{
    AeadCore, Aes256Gcm, KeyInit,
    aead::{AeadMut, OsRng},
};
use shared::{crypto::Encryption, types::ClientConfig};
use tokio::{fs, io};

use crate::{builder::running_dir, types::WhitelistedClient};

pub async fn main(config: &ClientConfig, path_str: &str) -> Result<WhitelistedClient, io::Error> {
    let key = Aes256Gcm::generate_key(OsRng); // make a key from random bytes
    let mut crypt: Encryption = Encryption::new(&key); // create Encryption struct with the key

    let mut client_path = running_dir(); // find out where the server executable is located
    client_path.push("stub.dat"); // append the client filename to get the full path to the client stub binary

    let mut client_bin = fs::read(&client_path).await?; // read client binary if possible

    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // nonce for encrypting config
    let config_data = bincode::encode_to_vec(&config, bincode::config::standard()).unwrap(); // convert config struct to bytes
    let ciphertext = crypt
        .cipher
        .encrypt(&nonce, config_data.as_slice()) // encrypt the config using the nonce and the cipher from the Encryption struct
        .unwrap();

    // convert all of these to bytes
    let key = crypt._key.to_vec();
    let whitelist_key = crypt._key.clone().into();
    let nonce_vec = nonce.to_vec();

    let mut patch_str = Vec::new();
    let total_len = (key.len() + nonce_vec.len() + ciphertext.len()) as u32;

    // push all the bytes onto the newly made Vec to merge them all
    patch_str.extend_from_slice(&total_len.to_le_bytes());
    patch_str.extend_from_slice(&key);
    patch_str.extend_from_slice(&nonce_vec);
    patch_str.extend_from_slice(&ciphertext);

    if patch_str.len() > 4096 {
        return Err(io::ErrorKind::FileTooLarge.into());
    };

    // mirror the 4kb of blank space that should exist in the client
    let client_empty = vec![0xAA; 4096];
    let client_offset = client_bin
        .windows(client_empty.len()) // look within 4kb
        .position(|window| window == &client_empty) // where 4kb of data matches our 4kb of 0xAA
        .ok_or("Couldn't find patch slot within stub bin!")
        .unwrap();

    client_bin[client_offset..client_offset + patch_str.len()].copy_from_slice(&patch_str); // copy our config
    for i in patch_str.len()..4096 { // padding: everything else should stay as 0xAA
        client_bin[client_offset + i] = 0xAA;
    }

    match fs::write(Path::new(&path_str), client_bin).await { // save to the file path chose by the user in the save dialog
        Ok(_) => (),
        Err(_e) => return Err(io::ErrorKind::PermissionDenied.into()),
    };

    Ok(WhitelistedClient { // return a WhitelistedClient struct for the builder to save to the whitelist/settings file
        mutex: hex::encode(config.mutex),
        key: whitelist_key,
    })
}
