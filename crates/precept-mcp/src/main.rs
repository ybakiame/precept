//! One stdio bridge for every registered project: it serves the project whose
//! root holds the session's folder (the client's MCP roots, else its own
//! working directory) through that project's agent capability, and nothing
//! anywhere else. Only the daemon reads storage/repository data.
mod binding;
mod bridge;
mod protocol;
#[cfg(test)]
mod tests;
mod work_session;

use bridge::Bridge;
use clap::Parser;
use precept_ipc::{Error, Location, MAX_FRAME};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Parser)]
struct Args {
    #[command(flatten)]
    location: Location,
}

// Bounded newline framing; unlike read_line this cannot allocate an unbounded line.
async fn line(reader: &mut BufReader<tokio::io::Stdin>) -> std::io::Result<Option<Vec<u8>>> {
    let mut data = Vec::new();
    let mut oversized = false;
    loop {
        let buf = reader.fill_buf().await?;
        if buf.is_empty() {
            return if data.is_empty() && !oversized {
                Ok(None)
            } else if oversized {
                Ok(Some(Vec::new()))
            } else {
                Ok(Some(data))
            };
        }
        let n = buf
            .iter()
            .position(|b| *b == b'\n')
            .map(|i| i + 1)
            .unwrap_or(buf.len());
        let done = buf[n - 1] == b'\n';
        if data.len() + n > MAX_FRAME {
            oversized = true;
            data.clear();
        } else if !oversized {
            data.extend_from_slice(&buf[..n]);
        }
        reader.consume(n);
        if done {
            return Ok(Some(data));
        }
    }
}

async fn run(args: Args) -> precept_ipc::Result<()> {
    args.location.root()?;
    let mut bridge = Bridge::new(args.location);
    // Until the client names its workspace folders, the working directory
    // the client started this server in decides.
    let cwd: Vec<_> = std::env::current_dir().into_iter().collect();
    bridge.bind(&cwd).await;
    let mut input = BufReader::new(tokio::io::stdin());
    let mut output = tokio::io::stdout();
    while let Some(raw) = line(&mut input).await.map_err(|_| Error::Unavailable)? {
        let response = match serde_json::from_slice::<Value>(&raw) {
            Ok(v) => bridge.handle(v).await,
            Err(_) => Some(protocol::error(
                Value::Null,
                -32700,
                "Malformed input: JSONDecodeError",
            )),
        };
        if let Some(value) = response {
            let mut raw = serde_json::to_vec(&value).map_err(|_| Error::Unavailable)?;
            if raw.len() > MAX_FRAME {
                raw = serde_json::to_vec(&protocol::error(
                    value["id"].clone(),
                    -32603,
                    "Response too large",
                ))
                .map_err(|_| Error::Unavailable)?;
            }
            raw.push(b'\n');
            output
                .write_all(&raw)
                .await
                .map_err(|_| Error::Unavailable)?;
            output.flush().await.map_err(|_| Error::Unavailable)?;
        }
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run(Args::parse()).await {
        eprintln!("precept-mcp: {error}");
        std::process::exit(2);
    }
}
