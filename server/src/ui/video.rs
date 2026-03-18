use std::time::{Duration, Instant};

use egui::{Image, PointerState, Sense, Slider, TextureHandle, Ui};
use shared::{
    commands::{CaptureCommand, CaptureQuality, CaptureType, Command},
    input::{InputType, ModifierKeys, MouseInputType},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    manager::types::UiManagerCommand,
    ui::client::types::{ClientViewCapture, ClientViewInputState},
};

pub fn render(
    ui: &mut Ui,
    capture_type: CaptureType, // important! we open a window for each capture type
    mutex: &String,
    sender: &UnboundedSender<UiManagerCommand>,
    input_state: &mut ClientViewInputState, // input simulation state (mouse/kbd)
    capture: &mut ClientViewCapture,
    capturing: &mut bool, // are we capturing? stores the button state
    texture: &mut Option<TextureHandle>, // paint frames to this handle
    selection: u32, // chosen camera or monitor index. uses list from computer_info payload
) {
    //////////////////////////////////////
    // quality toggle
    ui.horizontal(|ui| { // put everything within here on the same row
        if *capturing {
            if ui.button("⏹  Stop").clicked() {
                println!("[*] sending CaptureCommand::Stop");
                let _ = sender.send(UiManagerCommand::SendCommand(
                    mutex.clone(), // tell manager to send command to client, saying 'stop capturing this type'
                    Command::Capture(CaptureCommand::Stop, capture_type.clone()),
                ));
                *capturing = false; // update state to change button
            }
        } else {
            if ui.button("▶  Start").clicked() {
                println!("[*] sending CaptureCommand::Start");
                let _ = sender.send(UiManagerCommand::SendCommand(
                    mutex.clone(),
                    Command::Capture( // manager -> client "start capturing this type with these settings!"
                        CaptureCommand::Start(selection, capture.quality.clone()),
                        capture_type.clone(),
                    ),
                ));
                *capturing = true; // again, update state to change button
            };
        }
        ui.add_enabled_ui(!*capturing, |ui| { // only enabled when not capturing
            ui.horizontal(|ui| { // same row as button
                ui.label("Quality: ");
                ui.radio_value(&mut capture.quality, CaptureQuality::Quality, "Slow");
                ui.radio_value(&mut capture.quality, CaptureQuality::Speed, "Fast");
            });
        });
    });

    if let Some(ref image_data) = capture.data { // if we have a frame to render...
        let available_size = ui.available_size(); // see how much space we have to put the image
        let image_size = egui::Vec2::new(image_data.width() as f32, image_data.height() as f32); // how big is the image

        let min_scale = 0.1; // how small we can make the image
        let max_scale_for_space =  // calculate how much we can scale up
            (available_size.x / image_size.x).min(available_size.y / image_size.y) * 0.95;
        let max_scale = if capture.quality == CaptureQuality::Quality {
            max_scale_for_space.max(2.0) // if quality is higher, make it possible to scale up more
        } else {
            max_scale_for_space.max(1.0) // if lower, no point scaling up when quality sucks.
        };

        ui.add(
            Slider::new(&mut capture.scale, min_scale..=max_scale) // this controls the size of the frame
                .text("Scale")
                .step_by(0.05),
        );
    } else {
        let max_scale = if capture.quality == CaptureQuality::Quality {
            capture.max_scale
        } else {
            1.0 // different values when prioritising quality
        };
        ui.add(Slider::new(&mut capture.scale, 0.25..=max_scale).text("Scale"));
    }

    if let Some(ref image) = capture.data { // let's grab this handle again, but to paint
        if texture.is_none() { // in the case there is no texture...
            *texture = Some(ui.ctx().load_texture(
                format!("screen_{}", mutex.clone()),
                image.clone(), // load the image data into a paintable texture that we can render. we make one
                Default::default(),
            ));
        };
        if let Some(texture) = texture.as_ref() { // we should have this texture now
            let available_size = ui.available_size();
            let image_size = texture.size_vec2();

            let max_scale_x = available_size.x / image_size.x;
            let max_scale_y = available_size.y / image_size.y;
            let max_reasonable_scale = (max_scale_x.min(max_scale_y) * 0.9).max(0.1);
            capture.scale = capture.scale.clamp(0.1, max_reasonable_scale.max(2.0));

            let display_size = image_size * capture.scale;

            let image = ui // adding the actual frame to the window
                .add(Image::new(texture).fit_to_exact_size(display_size))
                .interact(Sense::click());

            // keyboard events will go in here because youll be hovering over the screen so it makes sense to forward
            if capture_type == CaptureType::Screen && *capturing && input_state.active {
                let pointer_pos = ui.ctx().input(|i| i.pointer.latest_pos()); // use this so it handles position even when clicking
                let pointer_down = ui.ctx().input(|i| i.pointer.any_down());

                if let Some(pos) = pointer_pos {
                    let should_update_position = match input_state.last_update {
                        Some(last)
                            if Instant::now().duration_since(last) < Duration::from_millis(50) =>
                        {
                            false
                        }
                        _ => true,
                    };

                    if should_update_position {
                        // KEYBOARD ///////////
                        ui.ctx().input(|i| {
                            for e in &i.events {
                                if let egui::Event::Key {
                                    key,
                                    physical_key: _,
                                    pressed,
                                    repeat,
                                    modifiers,
                                } = e
                                {
                                    if *repeat {
                                        return;
                                    };
                                    println!("{}", key.name());
                                    println!("{:?}", modifiers);

                                    let _ = sender.send(UiManagerCommand::SendCommand(
                                        mutex.clone(),
                                        Command::Input(InputType::Key(
                                            pressed.clone(),
                                            key.name().to_string(),
                                            ModifierKeys {
                                                ctrl: modifiers.ctrl,
                                                shift: modifiers.shift,
                                                alt: modifiers.alt,
                                            },
                                        )),
                                    ));

                                    return; // exit early after handling one key event
                                }
                            }
                        });
                        ///////////////////////

                        let top_left = image.rect.min;
                        let local_pos = pos - top_left;
                        let mut scale = 4.0;
                        if capture.quality == CaptureQuality::Quality {
                            scale = 2.0;
                        }
                        let (remote_width, remote_height) =
                            (image_size.x as f32 * scale, image_size.y as f32 * scale);

                        let mouse_pos = (
                            ((local_pos.x * scale) / capture.scale.clamp(0.0, 1.0))
                                .clamp(0.0, remote_width),
                            ((local_pos.y * scale) / capture.scale.clamp(0.0, 1.0))
                                .clamp(0.0, remote_height),
                        );

                        if let Some((last_x, last_y)) = input_state.last_position {
                            if mouse_pos != (last_x, last_y) {
                                input_state.last_position = Some(mouse_pos);
                                // println!("move mouse to {}x{}", mouse_pos.0, mouse_pos.1);
                                let _ = sender.send(UiManagerCommand::SendCommand(
                                    mutex.clone(),
                                    Command::Input(InputType::MouseMove(mouse_pos)),
                                ));
                                input_state.last_update = Some(Instant::now());
                            }
                        } else {
                            input_state.last_position = Some(mouse_pos);
                        }
                    }
                }

                if pointer_down && input_state.clicking == false {
                    // println!("[*] MOUSE DOWN!");

                    let mut input_type = MouseInputType::Left;
                    let pointer_state = PointerState::default();
                    if pointer_state.secondary_down() {
                        input_type = MouseInputType::Right;
                    }

                    let _ = sender.send(UiManagerCommand::SendCommand(
                        mutex.clone(),
                        Command::Input(InputType::MouseDown(input_type)),
                    ));
                    input_state.clicking = true;
                }
                if !pointer_down && input_state.clicking == true {
                    // println!("[*] MOUSE UP!");

                    let mut input_type = MouseInputType::Left;
                    let pointer_state = PointerState::default();
                    if pointer_state.secondary_released() {
                        input_type = MouseInputType::Right;
                    }

                    let _ = sender.send(UiManagerCommand::SendCommand(
                        mutex.clone(),
                        Command::Input(InputType::MouseUp(input_type)),
                    ));
                    input_state.clicking = false;
                }
            }
        }
    }
}
