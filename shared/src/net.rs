use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use std::mem::size_of;

pub async fn read<R>(reader: &mut R) -> std::io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{

    let mut len_buf = [0u8; size_of::<u64>()]; // first we are told how big we should expect data to be
    reader.read_exact(&mut len_buf).await?; // get from tcp stream

    let len = usize::from_le_bytes(len_buf); // convert to readable number

    if len > 128 * 1024 * 1024 { // happens in rare cases because client<->server fall out of sync
        println!("[x] dropping super huge payload");
        return Err(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            "Payload too large!!",
        ));
    }

    let mut buf = vec![0u8; len]; // prepare a vec that we fill with the data
    reader.read_exact(&mut buf).await?; // feed tcp stream into the vec
    println!("[v] got payload (len: {}KB, size: {}kb)", len / 1024, buf.len() / 1024);
    Ok(buf) // return the data as bytes
}

pub async fn write<W>(writer: &mut W, data: &[u8]) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let len_bytes = (data.len() as u64).to_le_bytes(); // calculate size of data
    writer.write_all(&len_bytes).await?; // tell size of data to peer
    writer.write_all(data).await?; // then write the data
    writer.flush().await?;
    println!("[v] sent payload (size: {}KB)", data.len() / 1024);
    Ok(()) // nothing to return
}
