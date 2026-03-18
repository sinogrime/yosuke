use std::path::PathBuf;

use crate::{
    SettingsPointer, settings,
    types::{UiBuilderMessage, mouthpieces::BuilderMouthpiece},
};
use aes_gcm::aead::rand_core::{self, RngCore};
use rfd::FileDialog;
use shared::types::ClientConfig;

mod build;

fn mutex() -> Result<[u8; 8], Box<dyn std::error::Error>> {
    let mut mutex_buf = [0u8; 8];
    rand_core::OsRng.fill_bytes(&mut mutex_buf);
    Ok(mutex_buf)
}
pub fn running_dir() -> PathBuf {
    let exe_path = std::env::current_exe().expect("Failed to get current_exe");

    #[cfg(target_os = "macos")]
    {
        // /Yosuke.app/Contents/MacOS/server → /Yosuke.app
        exe_path
            .parent() // MacOS
            .and_then(|p| p.parent()) // Contents
            .and_then(|p| p.parent()) // .app
            .and_then(|p| p.parent()) // parent folder
            .map(|p| p.to_path_buf())
            .expect("Failed to resolve .app bundle path")
    }

    #[cfg(not(target_os = "macos"))]
    {
        // /Yosuke.exe
        exe_path
            .parent()
            .map(|p| p.to_path_buf())
            .expect("Failed to resolve executable directory")
    }
}

pub async fn main(settings: SettingsPointer, mut mouthpiece: BuilderMouthpiece) {
    println!("[*] builder spawned"); // this code runs in its own thread, like a mini-server
    while let Some(command) = mouthpiece.from_ui.recv().await {
        match command {
            // takes a BuilderSettings struct as arguments
            UiBuilderMessage::Build(builder_settings) => { // did we get asked to build?
                println!("[*] generating mutex");
                let mutex = mutex().unwrap(); // randomly generate a mutex, unique per client
                let config = ClientConfig {
                    mutex: mutex, // the mutex we just created
                    address: builder_settings.address, // get from arguments
                    port: builder_settings.port, // get from arguments
                };
                println!("[*] opening save file dialog");
                if let Some(output_path) = FileDialog::new() // use the 'rfd' library to open save dialog
                    .add_filter("Executable", &["exe"])
                    .set_directory(running_dir())
                    .save_file()
                {
                    let out_path = output_path.to_str().unwrap(); // get the path the user chose
                    println!("[*] saving to:\n    {}", out_path);

                    match build::main(&config, out_path).await { // code moved to other function
                        Ok(client) => {
                            let mut _settings = settings.lock().await; // lock the settings variable so we can write
                            _settings.whitelist.push(client); // add the new client to the whitelist
                            println!("[*] added mutex {} to whitelist", hex::encode(mutex));
                            match settings::save(&*_settings).await { // save the settings to a file
                                Ok(_) => {
                                    println!("[*] settings saved");
                                }
                                Err(e) => {
                                    println!("[!] error saving settings: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            println!("[!] error building: {}", e);
                        }
                    }
                }
            }
        }
    }
}
