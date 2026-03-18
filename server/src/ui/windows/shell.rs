use crate::{manager::types::UiManagerCommand, ui::client::ClientView};
use egui::{Align, Frame, Key, Layout, RichText, ScrollArea, TextEdit, TextStyle, Ui};
use shared::commands::Command;

pub fn render(view: &mut ClientView, ui: &mut Ui) {
    Frame::new() // create a frame to fill the output with
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(8.0) // round corners look nice
        .inner_margin(4.0) // padding for text so it isn't right against the edge
        .show(ui, |ui| {
            ui.set_height(ui.available_height() - 48.0); // leave room for input line below us
            ScrollArea::vertical() // make the output scrollable
                .auto_shrink([false, false]) // fill the frame
                .show(ui, |ui| {
                    ui.label(
                        // create a RichText label that contains the output from the client process
                        RichText::new(&view.state.powershell.output)
                            .monospace()
                            .size(10.0) // small but readable, fits the window
                            .color(ui.visuals().text_color())
                            .text_style(TextStyle::Monospace), // monospace because it emulates terminal feeling
                    )
                });
        });

    ui.add_space(2.0); // just a little bit of padding

    let editor = ui.add(
        TextEdit::singleline(&mut view.state.powershell.input)
            .code_editor() // unforunately using multiline didn't allow for syntax highlighting
            // using singleine makes it seem more like a 'run' prompt which was the initial idea
            .desired_width(ui.available_width()), // fill to width of window
    );
    let unfocused = editor.lost_focus(); // check if typing within input box

    ui.add_space(2.0); // padding, again

    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.horizontal(|ui| {
            if (ui.button("Send").clicked()) // right to left, so this button comes first
                || (unfocused && ui.input(|i| i.key_pressed(Key::Enter)))
            // TLDR: if the send button was clicked, or we pressed enter (which unfocuses the input box)...
            {
                let _ = view.sender.send(UiManagerCommand::SendCommand( // send command to manager
                    view.mutex.clone(), // this is our mutex, so you know who to send this to
                    Command::PowerShell( // this is the command we want to run and the data it requires
                        view.state.powershell.input.clone(), // here is the input
                        //  ^ this gets cloned because it can't be moved when it's still used by the TextEdit
                        view.state.powershell.powershell,
                    ),
                ));
                view.state.powershell.input.clear(); // wipe the input box, ready for something else to be typed
                if unfocused {
                    editor.request_focus(); // bring our focus back to the input box after sending for convenience
                };
            };

            // representing the boolean by showing two options, one for PowerShell, one for cmd.
            ui.radio_value(&mut view.state.powershell.powershell, true, "PowerShell");
            ui.radio_value(
                &mut view.state.powershell.powershell,
                false,
                "Command Prompt",
            );
        });
    });

    ui.add_space(2.0);
}
