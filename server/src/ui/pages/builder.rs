use egui::{Align, Frame, Grid, Layout, RichText, ScrollArea, TextEdit, TextStyle, Ui, vec2};

use crate::{
    types::{BuilderSettings, UiBuilderMessage},
    ui::view::View,
};

pub fn render(view: &mut View, ui: &mut Ui) {
    ui.heading("Builder"); // heading to distinguish page, as if content didn't make it obvious

    Grid::new("builder_grid") // we put all the options in a grid to organise them
        .num_columns(2) // two options, two possible columns
        .spacing([10.0, 4.0]) // add spacing between the columns/rows for readability
        .show(ui, |ui| {
            ui.label("Address:"); // this is the field where the IP or address goes
            ui.add(TextEdit::singleline(&mut view.state.builder.address).desired_width(128.0));
            // whatever the user types gets stored immediately to the builder.address field of our state
            ui.end_row();

            ui.label("Port:"); // where the port of the server will go
            ui.add(TextEdit::singleline(&mut view.state.builder.port).desired_width(64.0));
            // again, user input is stored immediately to builder.port 
            ui.end_row();

            if ui.button("🔧  Build").clicked() { // when this button is clicked
                // we check if the value in the port field is a valid number
                if let Ok(num) = view.state.builder.port.parse::<u16>() {
                    let _ = // do not store the result
                        view.mouthpiece
                            .to_builder // get our mouthpiece to communicate with the builder thread
                            .send(UiBuilderMessage::Build(BuilderSettings { // ask it to build us a new client
                                // with the following settings:
                                address: view.state.builder.address.clone(), // cloned because we can't MOVE this field
                                port: num,
                            }));
                } else {
                    view.state
                        .notifications // queue a notification if the port was invalid
                        .error("Invalid port in Builder, cannot build!") // must look like an error
                        .duration(Some(std::time::Duration::from_secs(5))); // show it for 5 secs, long enough to be readable
                };
            }
        });

    // similarly to the sessions page, we want to show the logs of the builder thread in a scrollable area
    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        let size = ui.available_size() - vec2(16.0, 16.0);
        Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .corner_radius(8.0)
            .inner_margin(6.0)
            .show(ui, |ui| {
                ui.set_min_size(size);
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&view.state.logs.builder.join("\n"))
                                .monospace()
                                .size(12.0)
                                .text_style(TextStyle::Monospace),
                        )
                    })
            })
    });
}
