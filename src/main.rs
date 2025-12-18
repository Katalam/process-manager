use clap::Parser;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::signal;
use tokio_util::sync::CancellationToken;

#[derive(Parser, Debug)]
#[command(author = "Katalam", version = "v1.0.0", about = "Run multiple Laravel queue workers with graceful shutdown")]
struct Args {
    #[arg(short, long, default_value_t = 2)]
    count: u32,

    #[arg(long)]
    no_herd: bool,

    #[arg(short, long, default_value_t = 60)]
    timeout: u64,

    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let token = CancellationToken::new();
    let mut handles = Vec::new();

    for i in 1..=args.count {
        let (program, mut cmd_args) = if args.no_herd {
            ("php", vec!["artisan".to_string(), "queue:listen".to_string()])
        } else {
            ("herd", vec!["php".to_string(), "artisan".to_string(), "queue:listen".to_string()])
        };

        if args.timeout != 60 {
            cmd_args.push("--timeout".to_string());
            cmd_args.push(args.timeout.to_string());
        }

        if args.verbose {
            cmd_args.push("-v".to_string());
        }

        let mut cmd = Command::new(program);
        cmd.args(&cmd_args);

        // Ensure child processes die if the Rust script is killed or crashes
        cmd.kill_on_drop(true);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());

        let mut child = cmd.spawn().expect("Failed to start worker");
        let stdout = child.stdout.take().expect("Failed to open stdout");
        let mut reader = BufReader::new(stdout).lines();

        let worker_token = token.clone();

        let handle = tokio::spawn(async move {
            println!("[{}] {}", i, format!("{} {}", program, cmd_args.join(" ")));

            loop {
                tokio::select! {
                    // Listen for new lines from the subprocess
                    result = reader.next_line() => {
                        match result {
                            Ok(Some(line)) => println!("[{}] {}", i, line),
                            Ok(None) => break, // Stream closed
                            Err(_) => break,
                        }
                    }
                    // Listen for the shutdown signal
                    _ = worker_token.cancelled() => {
                        println!("[{}] Received shutdown signal...", i);
                        // Explicitly kill the child process
                        let _ = child.kill().await;
                        break;
                    }
                }
            }
            println!("[{}] Cleaned up.", i);
        });

        handles.push(handle);
    }

    signal::ctrl_c().await?;
    println!("\nShutdown signal received. Stopping all workers...");

    token.cancel();

    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}