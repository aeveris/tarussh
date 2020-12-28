use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rand::{thread_rng, Rng};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{io, net, task, time};

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let client_count = Arc::new(AtomicU32::new(0));
    let socket = tokio::net::TcpListener::bind("0.0.0.0:2222").await?;
    println!("accepting connections");
    loop {
        // Just log failed connections, we don't care
        match socket.accept().await {
            Ok((s, _)) => {
                let ccount = client_count.clone();
                task::spawn(handle_client(s, ccount));
            }
            Err(e) => println!("error on connection! {}", e),
        }
    }
    Ok(())
}

async fn handle_client(mut stream: net::TcpStream, ccount: Arc<AtomicU32>) -> io::Result<()> {
    let mut buf = [0u8; 255];
    let prev = ccount.fetch_add(1, Ordering::Relaxed);
    let addr = stream.peer_addr()?;
    println!("new client! {} ({} currently)", addr.to_string(), prev + 1);
    stream.read(&mut buf).await?;
    loop {
        let len = gen_answer(&mut buf).await;
        if let Err(_) = stream.write_all(&buf[..len]).await {
            break;
        }
        time::sleep(Duration::from_secs(10)).await;
    }
    let prev = ccount.fetch_sub(1, Ordering::Relaxed);
    println!(
        "client disconnect! {} ({} remaining)",
        addr.to_string(),
        prev - 1
    );
    Ok(())
}

async fn gen_answer(target_buf: &mut [u8]) -> usize {
    let mut rng = thread_rng();

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
