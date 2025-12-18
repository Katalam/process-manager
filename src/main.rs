use clap::Parser;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::signal;
use tokio_util::sync::CancellationToken;

#[derive(Parser, Debug)]
#[command(author, version, about = "Run multiple Laravel queue workers with graceful shutdown")]
struct Args {
    #[arg(short, long, default_value_t = 2)]
    count: u32,

    #[arg(long)]
    no_herd: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let token = CancellationToken::new();
    let mut handles = Vec::new();

    for i in 1..=args.count {
        let mut cmd = if args.no_herd {
            let mut c = Command::new("php");
            c.args(["artisan", "queue:listen"]);
            c
        } else {
            let mut c = Command::new("herd");
            c.args(["php", "artisan", "queue:listen"]);
            c
        };

        // Ensure child processes die if the Rust script is killed or crashes
        cmd.kill_on_drop(true);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());

        let mut child = cmd.spawn().expect("Failed to start worker");
        let stdout = child.stdout.take().expect("Failed to open stdout");
        let mut reader = BufReader::new(stdout).lines();

        let worker_token = token.clone();

        let handle = tokio::spawn(async move {
            println!("[System] Worker #{} started.", i);

            loop {
                tokio::select! {
                    // Listen for new lines from the subprocess
                    result = reader.next_line() => {
                        match result {
                            Ok(Some(line)) => println!("[Worker {}] {}", i, line),
                            Ok(None) => break, // Stream closed
                            Err(_) => break,
                        }
                    }
                    // Listen for the shutdown signal
                    _ = worker_token.cancelled() => {
                        println!("[System] Worker #{} received shutdown signal...", i);
                        // Explicitly kill the child process
                        let _ = child.kill().await;
                        break;
                    }
                }
            }
            println!("[System] Worker #{} cleaned up.", i);
        });

        handles.push(handle);
    }

    signal::ctrl_c().await?;
    println!("\n[System] Shutdown signal received. Stopping all workers...");

    token.cancel();

    for handle in handles {
        let _ = handle.await;
    }

    println!("[System] All workers stopped. Exiting.");
    Ok(())
}