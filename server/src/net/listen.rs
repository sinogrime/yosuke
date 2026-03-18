use crate::{
    SettingsPointer,
    manager::types::ServerManagerMessage,
    net::handler,
    types::{mouthpieces::ServerMouthpiece, *},
};
use tokio::net::TcpListener;

pub async fn main(
    settings: SettingsPointer,
    mut mouthpiece: ServerMouthpiece,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("[*] server spawned"); // like the builder, runs on another thread
    let mut listener: Option<TcpListener> = None; // init with no listener
    // we use an Option because we can have no listener when it's not running

    loop {
        tokio::select! {
            msg = mouthpiece.from_ui.recv() => { // try to get a message from the UI mouthpiece
                println!("[*] server got a message from the ui");
                match msg { // what kind of message did we get?
                    Some(UiMessage::Listen) => { // are we being told to listen?
                        match TcpListener::bind("0.0.0.0:5317").await { // create a TcpListener
                            Ok(l) => { // if we are now listening on port 5317
                                println!("[*][listen()] tcp listening on port 5317");
                                listener = Some(l); // set our listener variable to the newly made listener
                                mouthpiece.to_ui.send(ServerMessage::Listening)?; // tell the UI we're listening
                            }
                            Err(e) => eprintln!("[x][listen()] {}", e), // error handling
                        }
                    }
                    Some(UiMessage::Shutdown) => { // are we being told to stop listening?
                        println!("[*][listen()] stop listening...");
                        mouthpiece.to_manager.send(ServerManagerMessage::ClearClients)?; // remove all clients from manager
                        listener = None; // set listener variable to None, discards it and frees memory
                        mouthpiece.to_ui.send(ServerMessage::Stopped)?; // tell UI we have stopped
                    }, None => {break Ok(())} // channel has closed, program is probably closing.
                }
            }
            // using tokio allows us to check two conditions simultaneously without blocking each other
            response = async { // so here we also listen for tcp connections when we have a listener.
                match &listener {
                    Some(l) => l.accept().await, // accept incoming tcp connection
                    None => core::future::pending().await, // do nothing if there is no TcpListener in the listener variable
                }
            } => {
                if let Ok((mut stream, addr)) = response { // if we got a connection...
                    println!("[*][listen()] new connection from {}", addr);
                    if let Ok(client) = handler::handshake(&mut stream, &settings).await { // try to authenticate with it. should be a client
                        let _ = mouthpiece.to_manager.send(ServerManagerMessage::ClientConnect(
                            client.clone(), // copy the whitelist info from the whitelist to the manager
                            stream          // move stream to the manager
                        ));
                    };
                }
            }
        }
    }
}
