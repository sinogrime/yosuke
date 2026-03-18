use eframe::{NativeOptions, run_native};
use egui::{IconData, ViewportBuilder};

use crate::types::mouthpieces::UiMouthpiece;

mod client;
mod pages;
mod switcher;
mod updates;
pub mod video;
mod view;
mod windows;

pub fn main(mouthpiece: UiMouthpiece) -> eframe::Result<()> {
    println!("[*] ui spawned");
    let options = NativeOptions { // 'eframe' options for spawning a window
        hardware_acceleration: eframe::HardwareAcceleration::Preferred, // elaborate on openGL issues
        viewport: ViewportBuilder::default() // creates an egui Viewport to render the UI in
            .with_inner_size([720.0, 560.0]) // basic medium-sized window
            .with_icon(load_icon()), // custom function for loading icon to show in taskbar
        ..Default::default()
    };

    run_native( // start desktop app
        "Yosuke",    // title of app
        options,

        // Box = dynamically sized. run_native expects a 'trait object'.
        // ^ (trait objects have unknown sizes when compiled, so we store them on the heap using a Box)
        // Ok is needed because we must return a Result.
        Box::new(move |_cc| Ok(Box::new(view::View::new(mouthpiece)))),
    )
}

fn get_icon() -> Vec<u8> { // loads an icon or png based on platform
    #[cfg(target_os = "macos")] // if we are on macOS, use a PNG
    {
        return include_bytes!("../../../assets/yosuke.png").to_vec();
    }
    #[cfg(not(target_os = "macos"))] // if we are not on macOS (assumably Windows), use an ICO
    {
        return include_bytes!("../../../assets/yosuke.ico").to_vec();
    }
}

fn load_icon() -> IconData {
    let (icon_rgba, icon_width, icon_height) = {
        let icon = &get_icon(); // use our function

        // The 'image' library creates an object we can use with eframe as an icon from bytes
        let image = image::load_from_memory(icon)
            .expect("Failed to open icon path")
            .into_rgba8();
        
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();

        (rgba, width, height) // Returning the tuple that our variable will be
    };

    eframe::egui::IconData { // Returning the struct that eframe expects from us for an icon
        rgba: icon_rgba, // Array of bytes
        width: icon_width,
        height: icon_height,
    }
}
