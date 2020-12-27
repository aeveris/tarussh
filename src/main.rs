use std::time::Duration;

use rand::{thread_rng, Rng};

use async_std::prelude::*;
use async_std::{io, net, task};

use futures::stream::StreamExt;

#[async_std::main]
#[allow(unused_must_use)]
async fn main() -> io::Result<()> {
    let socket = net::TcpListener::bind("0.0.0.0:2222").await?;
    socket
        .incoming()
        .for_each_concurrent(4096, |s| async move {
            if let Ok(s) = s {
                handle_client(s).await;
            }
        })
        .await;
    Ok(())
}

async fn handle_client(mut stream: net::TcpStream) -> io::Result<()> {
    let mut buf = [0u8; 255];
    stream.read(&mut buf).await?;
    let addr = stream.peer_addr()?;
    println!("new client! {}", addr.to_string());
    let mut rng = thread_rng();
    loop {
        let len = gen_answer(&mut rng, &mut buf).await;
        if let Err(_) = stream.write_all(&buf[..len]).await {
            break;
        }
        task::sleep(Duration::from_secs(10)).await;
    }
    println!("client disconnect! {}", addr.to_string());
    Ok(())
}

async fn gen_answer<R: Rng>(rng: &mut R, target_buf: &mut [u8]) -> usize {
    // The maximum allowed length with CR LF is 255 bytes, that leaves 253 without the line termination
    let max_length = 253;

    // Generate a random length for each answer and populate the buffer
    let len = rng.gen_range(3..=max_length);
    (0..len).for_each(|idx| target_buf[idx] = rng.gen_range(0x20..=0x7e)); // Valid range is from ' ' to '~'

    // The answer shouldn't start with "SSH-"
    if &target_buf[..4] == b"SSH-" {
        target_buf[0] = b'X';
    }

    // Append "\r\n" to end the line
    target_buf[len] = b'\r';
    target_buf[len + 1] = b'\n';
    len + 2
}
