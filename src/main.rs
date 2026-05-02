use anyhow::{Context, Result};
use bundt::router;
use clap::Parser;
use std::io::ErrorKind;
use std::process::ExitStatus;
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

    tokio::pin! {
        let router_future = router::run(
            BufReader::new(tokio::io::stdin()),
            tokio::io::stdout(),
            lsp_stdin,
            BufReader::new(lsp_stdout),
        );
    }

    // Race the router against the child process exiting.
    //
    // If the child exits with a non-zero code (unexpected crash), we log and
    // propagate the exit code immediately rather than waiting for the IDE to
    // close its connection (which might never happen).
    //
    // If the child exits cleanly (code 0), we let the router finish draining
    // any remaining in-flight frames before returning.
    let status: ExitStatus = tokio::select! {
        res = &mut router_future => {
            res?;
            child.wait().await.context("waiting for TS LSP")?
        }
        status = child.wait() => {
            let status = status.context("waiting for TS LSP")?;
            if !status.success() {
                let code = status.code().unwrap_or(1);
                eprintln!("bundt: TS LSP exited with status {code}");
                std::process::exit(code);
            }
            // LSP exited cleanly: drain any remaining output before we return.
            router_future.await?;
            status
        }
    };

    if !status.success() {
        let code = status.code().unwrap_or(1);
        eprintln!("bundt: TS LSP exited with status {code}");
        std::process::exit(code);
    }

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
