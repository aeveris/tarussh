use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rand::{thread_rng, Rng};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::{io, net, task, time};

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "tarussh", about = "A small SSH tarpit")]
struct TarOpts {
    #[structopt(
        long,
        short = "-l",
        default_value = "42",
        help = "Maximum length of sent headers. Clamped to [3, 253]"
    )]
    max_line_length: u8,

    #[structopt(long, short, default_value = "22", help = "Listening port")]
    port: u16,

    #[structopt(
        long,
        short,
        default_value = "10000",
        help = "Delay between messages in ms"
    )]
    delay: u32,

    #[structopt(
        long,
        short,
        default_value = "2048",
        help = "Maximum number of trapped clients"
    )]
    max_clients: u32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let opts: TarOpts = TarOpts::from_args();
    let socket = net::TcpListener::bind(("::0", opts.port)).await?;

    let client_count = Arc::new(AtomicU32::new(0));
    let should_stop = Arc::new(AtomicBool::new(false));

    println!("accepting connections on port {}", opts.port);
    println!("use CTRL-C for a graceful shutdown");

    let mut sigint_stream = signal(SignalKind::interrupt())?;
    let sstop = should_stop.clone();
    let ccount = client_count.clone();
    select! {
        _ = async move {
            sigint_stream.recv().await;
            sstop.store(true, Ordering::Relaxed);
            while ccount.load(Ordering::Relaxed) > 0 {
                time::sleep(Duration::from_millis(1000)).await;
            }
        } => (),
        _ = server(socket, opts, client_count, should_stop) => ()
    };
    Ok(())
}

async fn server(
    listener: net::TcpListener,
    opts: TarOpts,
    client_count: Arc<AtomicU32>,
    should_stop: Arc<AtomicBool>,
) {
    loop {
        if client_count.load(Ordering::Relaxed) < opts.max_clients {
            match listener.accept().await {
                Ok((s, addr)) => {
                    let prev = client_count.fetch_add(1, Ordering::Relaxed);
                    println!("new client! {} ({} currently)", addr.to_string(), prev + 1);
                    let ccount = client_count.clone();
                    let sstop = should_stop.clone();
                    task::spawn(handle_client(
                        s,
                        ccount,
                        opts.max_line_length.max(3).min(253),
                        opts.delay,
                        sstop,
                    ));
                }
                Err(e) => println!("error on connection! {}", e),
            }
        } else {
            time::sleep(Duration::from_millis(opts.delay as u64)).await;
        }
    }
}

async fn handle_client(
    mut stream: net::TcpStream,
    ccount: Arc<AtomicU32>,
    max_line_length: u8,
    delay: u32,
    should_stop: Arc<AtomicBool>,
) -> io::Result<()> {
    let mut buf = [0u8; 255];
    let addr = stream.peer_addr()?;
    stream.read(&mut buf).await?;
    while !should_stop.load(Ordering::Relaxed) {
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
