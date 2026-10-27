use tokio::io::AsyncWriteExt;

use logging::info;
use kernel::boot::{self, BootOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let server_mode = args.iter().any(|a| a == "--server");
    let connect_mode = args.iter().any(|a| a == "--connect");

    if connect_mode {
        let addr = args
            .iter()
            .skip_while(|a| a != &"--connect")
            .nth(1)
            .map(|s| s.as_str())
            .unwrap_or("127.0.0.1:7890");
        return tokio::runtime::Runtime::new()?.block_on(client_connect(addr));
    }

    if !server_mode {
        eprintln!(
            "Magma's interactive frontend lives in the `magma-gui` crate: run `cargo run -p magma-gui`, \
             or pass --server for headless mode."
        );
        return Ok(());
    }

    let addr = args
        .iter()
        .skip_while(|a| a != &"--server")
        .nth(1)
        .map(|s| s.as_str())
        .unwrap_or("127.0.0.1:7890");

    let boot = boot::bootstrap(BootOptions {
        interactive: false,
        reopen_last_project: false,
    }, None)?;

    info!("Starting Magma server on {addr} (headless)");
    let result = boot.runtime.block_on(async move {
        kernel::server::run_server(boot.editor, boot.bg_receiver, addr, None).await
    });
    drop(result);
    boot::shutdown(boot.runtime);
    Ok(())
}

async fn client_connect(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::net::TcpStream;

    let token = std::env::var("MAGMA_SERVER_TOKEN").map_err(|_| {
        "set MAGMA_SERVER_TOKEN to the token the server printed at startup".to_string()
    })?;
    let stream = TcpStream::connect(addr).await?;
    let (mut reader, mut writer) = stream.into_split();

    // Auth handshake before any op.
    writer
        .write_all(format!("auth {token}\n").as_bytes())
        .await?;
    {
        use tokio::io::AsyncReadExt as _;
        let mut handshake = vec![0u8; 256];
        let n = reader.read(&mut handshake).await?;
        let greeting = String::from_utf8_lossy(&handshake[..n]);
        if greeting.contains("unauthorized") {
            return Err(format!("server rejected the token: {greeting}").into());
        }
    }
    let mut stdout = tokio::io::stdout();
    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    let mut server_lines = BufReader::new(reader).lines();

    loop {
        tokio::select! {
            line = stdin.next_line() => {
                match line? {
                    Some(text) if !text.trim().is_empty() => {
                        writer.write_all(text.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            resp = server_lines.next_line() => {
                match resp? {
                    Some(line) => {
                        stdout.write_all(format!("{line}\n").as_bytes()).await?;
                        stdout.flush().await?;
                    }
                    None => {
                        logging::warn!("Server disconnected");
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}
