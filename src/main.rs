use anyhow::{Context, Result};
use bundt::router;
use clap::Parser;
use std::io::ErrorKind;
use tokio::io::BufReader;
use tokio::process::Command;

#[derive(Parser)]
#[command(about = "LSP proxy that adds Bun context to a downstream TypeScript LSP")]
struct Args {
    /// Path to the TypeScript LSP binary (e.g. vtsls)
    binary: String,
    /// Arguments forwarded to the TypeScript LSP
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    lsp_args: Vec<String>,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();

    let mut child = Command::new(&args.binary)
        .args(&args.lsp_args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| match e.kind() {
            ErrorKind::NotFound => anyhow::anyhow!("'{}': not found", args.binary),
            _ => anyhow::anyhow!("failed to spawn '{}': {}", args.binary, e),
        })?;

    let lsp_stdin = child.stdin.take().context("child has no stdin")?;
    let lsp_stdout = child.stdout.take().context("child has no stdout")?;

    router::run(
        BufReader::new(tokio::io::stdin()),
        tokio::io::stdout(),
        lsp_stdin,
        BufReader::new(lsp_stdout),
    )
    .await?;

    child.wait().await.context("waiting for TS LSP")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_binary_and_rest() {
        let args = Args::try_parse_from(["bundt", "vtsls", "--stdio"]).unwrap();
        assert_eq!(args.binary, "vtsls");
        assert_eq!(args.lsp_args, ["--stdio"]);
    }

    #[test]
    fn args_binary_only_empty_rest() {
        let args = Args::try_parse_from(["bundt", "vtsls"]).unwrap();
        assert_eq!(args.binary, "vtsls");
        assert!(args.lsp_args.is_empty());
    }

    #[test]
    fn args_no_binary_fails() {
        assert!(Args::try_parse_from(["bundt"]).is_err());
    }
}
