use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::{SettingsPointer, types::WhitelistedClient};

pub async fn handshake(
    stream: &mut TcpStream,
    settings: &SettingsPointer,
) -> Result<WhitelistedClient, std::io::Error> {
    println!("[*][handle()] waiting for handshake");
    let mut handshake_buf = [0u8; 5]; // how large the handshake packet from the client should be. 5 bytes
    stream.read_exact(&mut handshake_buf).await?; // read [what should be] the handshake in from the client
    if handshake_buf != [0x0a, 0xee, 0x7c, 0x9b, 0x32] { // did we not get these exact magic bytes?
        //                                                  this means the connection is not from a client
        println!("[x][handle()] invalid handshake");
        return Err(std::io::ErrorKind::InvalidData.into()); // throw an error, which will close the connection
    } else {
        println!("[v][handle()] valid handshake");
        stream.write_all(&[0x32, 0x9b, 0x7c, 0xee, 0x0a]).await?; // we write back to the client to let it know
        //                                                                         it has connected to a server.

        println!("[*][handle()] waiting for mutex...");
        let mut mutex_buf = [0u8; 8]; // how large a client mutex is
        stream.read_exact(&mut mutex_buf).await?; // the client sends its mutex to identify itself
        let mutex = hex::encode(mutex_buf); // bytes to string for printing and debugging
        println!("[v][handle()] got mutex: {}", mutex);

        let whitelist = &settings.lock().await.whitelist; // get our whitelist, lock mutex to access it
        if let Some(client) = whitelist.iter().cloned().find(|c| c.mutex == mutex) { // cost of cloning the whitelist is cheap
            //                                                ^ we look for this mutex we've been given within our whitelist
            println!("[v][handle()] mutex is whitelisted!");
            return Ok(client); // if we find it, we return the client's info to the caller, which will add it to the manager's list of clients
        } else {
            println!("[x][handle()] mutex is not whitelisted");
            return Err(std::io::ErrorKind::PermissionDenied.into()); // throw an error and disconnect the client
        }
    }
}
