use tokio::io::AsyncWriteExt;

use cli::Command;
use kernel::boot::{self, BootOptions};
use logging::info;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = match cli::parse(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(e) => {
            eprintln!("magma: {e}\nTry 'magma --help'.");
            std::process::exit(2);
        }
    };
    logging::install_panic_hook();
    if !matches!(command, Command::Help | Command::Version) {
        let log_path = kernel::state::persist::state_dir().join("magma.log");
        if let Err(e) = logging::init_file(&log_path) {
            eprintln!("magma: cannot open log file {}: {e}", log_path.display());
        }
    }
    match command {
        Command::Help => {
            print!("{}", cli::HELP);
            Ok(())
        }
        Command::Version => {
            println!("{}", cli::version_line());
            Ok(())
        }
        Command::Connect { addr } => {
            tokio::runtime::Runtime::new()?.block_on(client_connect(&addr))
        }
        Command::Server { addr } => run_server(addr),
        Command::Run(options) => {
            if let Some(level) = options.log_level {
                logging::set_max_level(level);
            }
            let launch = match cli::resolve(&options, &std::env::current_dir()?) {
                Ok(launch) => launch,
                Err(e) => {
                    eprintln!("magma: {e}");
                    std::process::exit(2);
                }
            };
            ui::run(ui::StartOptions {
                project: launch.project,
                files: launch.files,
                reopen_last: options.reopen_last,
            })
        }
    }
}

fn run_server(addr: String) -> Result<(), Box<dyn std::error::Error>> {
    let boot = boot::bootstrap(
        BootOptions {
            interactive: false,
            reopen_last_project: false,
        },
        None,
    )?;

    info!("Starting Magma server on {addr} (headless)");
    let result = boot.runtime.block_on(async move {
        kernel::server::run_server(boot.editor, boot.bg_receiver, &addr, None).await
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
