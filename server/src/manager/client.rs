use std::net::SocketAddr;

use shared::crypto::Encryption;
use tokio::{
    net::TcpStream,
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

pub enum ClientCommand {
    Write(Vec<u8>),
}
pub enum ClientResponse {
    Read(String, Vec<u8>),
    Disconnect(String),
}
pub struct Client {
    pub mutex: String,
    pub counter: u64,
    pub socket: SocketAddr,
    pub sender: UnboundedSender<ClientCommand>,
    pub handle: JoinHandle<()>, // tokio::task per client
}
pub struct ClientPassthroughMouthpiece {
    pub to_manager: UnboundedSender<ClientResponse>,
    pub from_manager: UnboundedReceiver<ClientCommand>,
}

pub async fn task(
    mutex: String,
    encryption: Encryption,
    stream: TcpStream,
    mut mouthpiece: ClientPassthroughMouthpiece,
) -> Result<(), std::io::Error> {
    let (read, write) = stream.into_split(); // split our tcp stream into two, one for reading and one for writing.
    //                                          this helps read and write at the same time without blocking
    let mut read = read.compat();
    let mut write = write.compat_write();

    loop {
        tokio::select! {
            // upon receiving data from a client over the network...
            stream_read = shared::net::read(&mut read) => { // using our net function, read in from our tcp stream.
                match stream_read {
                    Ok(buf) => { // decryption starts if we got data successfuly
                        let mut nonce = [0u8; 12]; // reserve space for nonce
                        nonce.copy_from_slice(&buf[..12]); // first 12 bytes of data is nonce
                        let buffer = &buf[12..]; // the rest is encrypted data
                        if let Ok(decrypted) = encryption.decrypt(&nonce, buffer) { // decrypt the data with the nonce
                            let _ = mouthpiece.to_manager.send(ClientResponse::Read(mutex.clone(), decrypted)); // send the decrypted data to the manager for processing,
                            //                                                                                     along with the mutex so the manager knows who sent it 
                        } else {
                            println!("decryption failed!!"); // very bitter error handling
                        }
                    },
                    Err(_e) => {
                        println!("[x] error reading data from client: {}", _e); // error handling a litle better
                        if _e.kind() != std::io::ErrorKind::FileTooLarge {
                            let _ = mouthpiece.to_manager.send(ClientResponse::Disconnect(mutex.clone())); // we will disconnect the client if something goes seriously wrong
                            return Err(_e);
                        }
                    }
                }
            }

            // upon receiving a command from the manager
            manager_read = mouthpiece.from_manager.recv() => {
                if let Some(command) = manager_read {
                    match command {
                        ClientCommand::Write(buf) => { // if we are told to send data to the client
                            if let Ok((nonce, encrypted)) = encryption.encrypt(&buf) // we encrypt it, getting back a nonce and encrypted data
                            {
                                let mut payload = Vec::new(); // make a payload that merges the nonce and encrypted data into one large byte array
                                payload.extend_from_slice(&nonce);
                                payload.extend_from_slice(&encrypted);

                                let _ = shared::net::write(&mut write, &payload).await; // send it off to the client using our net function

                            } else {
                                println!("[x] failed to write to client");
                                return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Failed to write to client")); // throw an error if encryption fails,
                                //                                                                                         which will cause the client to disconnect
                            }
                        }
                    }
                }
            }
        }
    }
}
