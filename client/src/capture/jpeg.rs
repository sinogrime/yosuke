use image::{ImageBuffer, Rgb, imageops::FilterType};
use jpeg_encoder::Encoder;
use shared::commands::VideoPacket;

pub struct FrameSize {
    pub width: u32,
    pub height: u32,
}

pub fn encode_fast(
    frame: &[u8], // pointer to bytes as data
    from: FrameSize, // w&h
    to: FrameSize, // w&h
    quality: u8, // 0-100, higher=better, takes longer though
    rgb_buffer: &mut Vec<u8>, // we write here, which is why its a pointer
) -> VideoPacket {
    rgb_buffer.clear(); // wipe first, just in caes
    rgb_buffer.extend(
        frame // fill with our frame
            .chunks_exact(4)
            .flat_map(|bgra| [bgra[2], bgra[1], bgra[0]]), // make sure pixels are in correct order
    );

    // let img makes an image object that is used to resize and encode to JPEG
    let img = ImageBuffer::<Rgb<u8>, _>::from_raw(from.width, from.height, rgb_buffer.clone()).unwrap();
    let final_img = if to.width != from.width || to.height != from.height {
        // resize the image in case the target size is different from the original size
        // quality is usually lost here for performance
        image::imageops::resize(&img, to.width, to.height, FilterType::Nearest)
    } else {
        img // nothing needed to do so just return what we already had lol
    };

    let mut jpeg_data = Vec::new(); // get ready to contain jpeg data!
    let encoder = Encoder::new(&mut jpeg_data, quality); // prepare jpeg encoder from 'jpeg_encoder' crate
    encoder
        .encode(
            final_img.as_raw(), // bytes of image object
            to.width as u16,
            to.height as u16,
            jpeg_encoder::ColorType::Rgb,
        )
        .unwrap();

    VideoPacket { // this is what we return, which will be sent to the server
        data: jpeg_data,
        width: to.width,
        height: to.height,
    }
}
