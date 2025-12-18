use clap::Parser;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::signal;
use tokio_util::sync::CancellationToken;

#[derive(Parser, Debug)]
#[command(author = "Katalam", version = "v1.1.0", about = "Laravel Queue Runner")]
struct Args {
    /// The queue definitions in pairs: <name> <count> (e.g., default 2 critical 1)
    #[arg(value_name = "QUEUE_PAIRS")]
    queue_definitions: Vec<String>,

    #[arg(long)]
    no_herd: bool,

    /// Use queue:work instead of queue:listen
    #[arg(long)]
    use_work: bool,

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

    let mut queues = Vec::new();
    let mut i = 0;
    while i < args.queue_definitions.len() {
        let name = args.queue_definitions[i].clone();
        let count = args.queue_definitions.get(i + 1)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);

        queues.push((name, count));

        i += 2;
    }

    if queues.is_empty() {
        queues.push(("default".to_string(), 2));
    }

    let max_q_len = queues.iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0);

    let command_type = if args.use_work { "queue:work" } else { "queue:listen" };
    let mut global_worker_id = 0;

    for (queue_name, count) in queues {
        for _ in 0..count {
            global_worker_id += 1;
            let worker_id = global_worker_id;
            let q_name = queue_name.clone();

            let (program, mut cmd_args) = if args.no_herd {
                ("php", vec!["artisan".to_string(), command_type.to_string()])
            } else {
                ("herd", vec!["php".to_string(), "artisan".to_string(), command_type.to_string()])
            };

            cmd_args.push("--queue".to_string());
            cmd_args.push(q_name.clone());

            if args.timeout != 60 {
                cmd_args.push("--timeout".to_string());
                cmd_args.push(args.timeout.to_string());
            }

            if args.verbose {
                cmd_args.push("-v".to_string());
            }

            let mut cmd = Command::new(program);
            cmd.args(&cmd_args);
            cmd.kill_on_drop(true);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::inherit());

            let mut child = cmd.spawn().expect("Failed to start worker");
            let stdout = child.stdout.take().expect("Failed to open stdout");
            let mut reader = BufReader::new(stdout).lines();
            let worker_token = token.clone();

            let cmd_display = format!("{} {}", program, cmd_args.join(" "));

            let handle = tokio::spawn(async move {
                println!("\x1b[2m[{}] Spawning {}\x1b[0m", worker_id, cmd_display);

                loop {
                    tokio::select! {
                        result = reader.next_line() => {
                            match result {
                                Ok(Some(line)) => {
                                    if !line.trim().is_empty() {
                                    println!(
                                            "\x1b[2m[{:02}] {:<width$} |\x1b[0m {}",
                                            worker_id,
                                            q_name,
                                            line,
                                            width = max_q_len
                                        );
                                    }
                                }
                                Ok(None) | Err(_) => break,
                            }
                        }
                        _ = worker_token.cancelled() => {
                            let _ = child.kill().await;
                            break;
                        }
                    }
                }
            });

            handles.push(handle);
        }
    }

    signal::ctrl_c().await?;
    println!("\n\x1b[33mShutdown signal received. Stopping {} workers...\x1b[0m", handles.len());

    token.cancel();
    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}