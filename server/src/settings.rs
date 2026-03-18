use bincode::{config, decode_from_slice, encode_to_vec};
use std::fs;

use crate::{builder::running_dir, types::Settings};

// return a Settings struct, the one found or a new one, or an error if we have a problem
pub async fn load() -> Result<Settings, Box<dyn std::error::Error>> {
    let mut settings_path = running_dir(); // get the folder of the server executable
    settings_path.push("settings.dat"); // add 'settings.dat' to that folder path
    match fs::read(settings_path) { // read the settings.dat file
        Ok(settings_content) => { // if it exists
            let (settings, _length): (Settings, usize) =
                decode_from_slice(settings_content.as_slice(), config::standard())?; // use bincode to convert to struct
            println!("[v] loaded settings.dat");
            return Ok(settings); // return the settings struct
        }
        Err(_e) => { // if it doesn't exist...
            println!("[*] could not load settings, reset to default!");
            return Ok(Settings::default()); // return a newly made settings struct
        }
    }
}

// write to settings.dat file
pub async fn save(settings: &Settings) -> Result<(), Box<dyn std::error::Error>> {
    let encoded_settings = encode_to_vec(settings, config::standard())?; // use bincode to convert to bytes
    let mut settings_path = running_dir(); // get folder of server exe
    settings_path.push("settings.dat");
    fs::write(settings_path, encoded_settings)?; // write to the settings.dat file
    println!("[v] saved settings.dat");
    Ok(()) // return nothing if successful
}
