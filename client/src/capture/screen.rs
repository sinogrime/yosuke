use shared::commands::{
    BaseResponse, CapturePacket, CaptureQuality, CaptureType, Response, VideoPacket,
};
use smol::channel::Sender;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[cfg(windows)]
use winapi;
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
};

use crate::{
    capture::jpeg::{FrameSize, encode_fast},
    handler::send,
};

struct CaptureHandler {
    id: u64,
    tx: Sender<Vec<u8>>,
    running: Arc<AtomicBool>,
    quality: CaptureQuality,
    target_width: usize,
    target_height: usize,
    frame_vec: Vec<u8>,
    rgb_buf: Vec<u8>,
}

impl GraphicsCaptureApiHandler for CaptureHandler {
    type Flags = (
        u64,
        Sender<Vec<u8>>,
        Arc<AtomicBool>,
        CaptureQuality,
        usize,
        usize,
        usize,
    );
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (id, tx, running, quality, target_width, target_height, initial_capacity) = ctx.flags;
        Ok(Self {
            id,
            tx,
            running,
            quality,
            target_width,
            target_height,
            frame_vec: Vec::with_capacity(initial_capacity),
            rgb_buf: Vec::new(),
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl, // not ours! belongs to windows_capture crate
    ) -> Result<(), Self::Error> {
        if !self.running.load(Ordering::SeqCst) { // if our atomic bool becomes false, we shut off
            capture_control.stop();
            return Ok(());
        }

        // frame dimensions
        let width = frame.width() as usize;
        let height = frame.height() as usize;

        let mut frame_buffer = frame.buffer()?; // raw framebuffer from windows screen capture
        let frame_data = frame_buffer.as_raw_buffer(); // okay. actually the raw framebuffer. sorry.

        self.stride(frame_data, width, height); // bugfix: capture was completely unusable without fixing screen 'stride'.
        //                                          pic is attached to NEA doc

        let mut jpeg_quality = 60; // good compromise of both quality and speed by default
        if self.quality == CaptureQuality::Quality {
            jpeg_quality = 80;  // we can go a little higher if the user wants
        }

        let packet: VideoPacket = encode_fast( // use our encode function to make a JPEG
            &self.frame_vec,
            FrameSize {
                // from
                width: width as u32,
                height: height as u32,
            },
            FrameSize {
                // to
                width: self.target_width as u32,
                height: self.target_height as u32,
            },
            jpeg_quality,
            &mut self.rgb_buf,
        );

        send(
            BaseResponse {
                id: self.id,
                response: Response::CapturePacket( // send off a JPEG to the server
                    CaptureType::Screen,
                    CapturePacket::Video(packet),
                ),
            },
            &self.tx,
        );

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        println!("[*] Capture session closed");
        Ok(())
    }
}

impl CaptureHandler {
    fn stride(&mut self, frame: &[u8], width: usize, height: usize) {
        self.frame_vec.clear();

        let expected_size = width * height * 4;

        if frame.len() == expected_size {
            self.frame_vec.extend_from_slice(frame);
            return;
        }

        let bytes_per_row = frame.len() / height;
        let expected_bytes_per_row = width * 4;

        for y in 0..height {
            let row_start = y * bytes_per_row;
            let row_end = row_start + expected_bytes_per_row;

            if row_end <= frame.len() {
                self.frame_vec.extend_from_slice(&frame[row_start..row_end]);
            }
        }
    }
}

pub fn main(
    id: u64,
    tx: Sender<Vec<u8>>,
    running: Arc<AtomicBool>,
    quality: CaptureQuality,
    device: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    // for Windows, we need to set the process DPI awareness to avoid scaling issues with high-DPI displays
    #[cfg(windows)]
    unsafe { // uses the windows API which is always unsafe, sadly
        winapi::um::winuser::SetProcessDpiAwarenessContext(
            winapi::shared::windef::DPI_AWARENESS_CONTEXT_SYSTEM_AWARE,
        );
    }

    let monitors = Monitor::enumerate().map_err(|e| e.to_string())?; // get all monitors present on the system
    let monitor = monitors
        .into_iter()
        .nth(device as usize) // get monitor based on index
        .ok_or("Monitor not found")?;

    let (width, height) = ( // get the dimensions of the monitor
        monitor.width().map_err(|e| e.to_string())? as usize,
        monitor.height().map_err(|e| e.to_string())? as usize,
    );

    let mut resize_factor = 4.0; // by default, downscale by 4x for more responsive capture. lower quality, can be unreadable!
    if quality == CaptureQuality::Quality { // if we want high quality capture instead, we downscale by 2x to preserve details
        resize_factor = 2.0;
    }
    let (target_width, target_height) = ( // calculate new size of the frame after resize
        (width as f32 / resize_factor) as usize,
        (height as f32 / resize_factor) as usize,
    );

    let initial_capacity = width * height * 4; // 4 bytes per pixel for BGRA8 format

    // we use the 'windows_capture' crate to capture, which requires Settings to be configured
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::Default,
        DrawBorderSettings::WithoutBorder, // Disable the yellow border
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8, // Try RGBA8 first
        ( // we add our own flags to passthrough to the capture handler here
            id, // command id
            tx, // channel to send frames to handler
            running.clone(), // atomic bool: state. running/not
            quality, // enum for quality/speed
            target_width, // resized width
            target_height, // resized height
            initial_capacity, // how much space we will use for capturing
        ),
    );

    // Start capture session
    CaptureHandler::start(settings)?; // passthrough to here
    println!("[*] capturer should be dropped now!!");

    Ok(())
}
