use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rand::{thread_rng, Rng};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{io, net, task, time};

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "tarussh", about = "A small SSH tarpit")]
struct TarOpts {
    #[structopt(
        long,
        short,
        default_value = "42",
        help = "Maximum length of sent headers. Clamped to [3, 253]"
    )]
    max_line_length: u8,

    #[structopt(long, short, default_value = "2222", help = "Listening port")]
    port: u16,

    #[structopt(
        long,
        short,
        default_value = "10000",
        help = "Delay between messages in ms"
    )]
    delay: u32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let opts = TarOpts::from_args();
    let max_line_length = opts.max_line_length.clamp(3, 253);
    let client_count = Arc::new(AtomicU32::new(0));
    let socket = net::TcpListener::bind(("::0", opts.port)).await?;
    println!("accepting connections");
    loop {
        match socket.accept().await {
            Ok((s, _)) => {
                let ccount = client_count.clone();
                task::spawn(handle_client(s, ccount, max_line_length, opts.delay));
            }
            Err(e) => println!("error on connection! {}", e),
        }
    }
    Ok(())
}

async fn handle_client(
    mut stream: net::TcpStream,
    ccount: Arc<AtomicU32>,
    max_line_length: u8,
    delay: u32,
) -> io::Result<()> {
    let mut buf = [0u8; 255];
    let prev = ccount.fetch_add(1, Ordering::Relaxed);
    let addr = stream.peer_addr()?;
    println!("new client! {} ({} currently)", addr.to_string(), prev + 1);
    stream.read(&mut buf).await?;
    loop {
        let len = gen_answer(&mut buf, max_line_length).await;
        if let Err(_) = stream.write_all(&buf[..len]).await {
            break;
        }
        time::sleep(Duration::from_millis(delay as u64)).await;
    }
    let prev = ccount.fetch_sub(1, Ordering::Relaxed);
    println!(
        "client disconnect! {} ({} remaining)",
        addr.to_string(),
        prev - 1
    );
    Ok(())
}

async fn gen_answer(target_buf: &mut [u8], max_line_length: u8) -> usize {
    let mut rng = thread_rng();

    // Generate a random length for each answer and populate the buffer
    let len = rng.gen_range(3..=max_line_length) as usize;
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
