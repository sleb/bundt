/// Test helper: reads LSP frames from stdin and echoes them back to stdout.
/// Exits 0 on clean EOF. Accepts an optional --exit-code <N> to override the
/// exit code (for testing unexpected subprocess exits in step 5).
use std::process;

use anyhow::Result;
use bundt::framing::{read_frame, write_frame};
use clap::Parser;
use tokio::io::{self, BufReader};

#[derive(Parser)]
struct Args {
    /// Exit with this code instead of 0
    #[arg(long, default_value_t = 0)]
    exit_code: i32,
    /// Exit after echoing this many frames (default: run until EOF)
    #[arg(long)]
    exit_after: Option<usize>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(e) = run(&args).await {
        eprintln!("lsp_echo error: {e}");
        process::exit(1);
    }
    process::exit(args.exit_code);
}

async fn run(args: &Args) -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut writer = stdout;
    let mut count = 0;

    while let Some(frame) = read_frame(&mut reader).await? {
        write_frame(&mut writer, &frame).await?;
        count += 1;
        if args.exit_after == Some(count) {
            break;
        }
    }

    Ok(())
}
