use anyhow::Result;
use tokio::io::{AsyncBufRead, AsyncWrite};

use crate::framing::{read_frame, write_frame};

/// Runs the bidirectional forward loop.
///
/// Takes ownership of all four I/O handles so that dropping a handle (e.g.
/// `lsp_in` when the IDE→LSP direction completes) closes the corresponding
/// pipe, which is the signal that causes the downstream process to exit.
pub async fn run(
    ide_in: impl AsyncBufRead + Unpin,
    ide_out: impl AsyncWrite + Unpin,
    lsp_in: impl AsyncWrite + Unpin,
    lsp_out: impl AsyncBufRead + Unpin,
) -> Result<()> {
    tokio::try_join!(
        forward(ide_in, lsp_in, "IDE→LSP"),
        forward(lsp_out, ide_out, "LSP→IDE"),
    )
    .map(|_| ())
}

async fn forward(
    mut reader: impl AsyncBufRead + Unpin,
    mut writer: impl AsyncWrite + Unpin,
    label: &'static str,
) -> Result<()> {
    loop {
        let frame = match read_frame(&mut reader).await {
            Ok(Some(f)) => f,
            Ok(None) => return Ok(()), // clean EOF
            Err(e) => {
                eprintln!("bundt: {label}: framing error ({e})");
                return Err(e);
            }
        };
        if serde_json::from_slice::<serde_json::Value>(&frame).is_err() {
            eprintln!("bundt: {label}: malformed JSON frame, skipping");
            continue;
        }
        write_frame(&mut writer, &frame).await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    use crate::framing::write_frame;

    // Unit tests call forward() directly via &mut references so the output
    // Vec stays accessible for inspection after the call.

    #[tokio::test]
    async fn ide_to_lsp_forwarded_unchanged() {
        let body = b"{\"method\":\"initialize\"}";
        let mut encoded = Vec::new();
        write_frame(&mut encoded, body).await.unwrap();
        let mut reader = BufReader::new(encoded.as_slice());
        let mut lsp_in = Vec::new();

        forward(&mut reader, &mut lsp_in, "IDE→LSP").await.unwrap();

        let received = read_frame(&mut BufReader::new(lsp_in.as_slice())).await.unwrap().unwrap();
        assert_eq!(received, body);
    }

    #[tokio::test]
    async fn lsp_to_ide_forwarded_unchanged() {
        let body = b"{\"id\":1,\"result\":{}}";
        let mut encoded = Vec::new();
        write_frame(&mut encoded, body).await.unwrap();
        let mut reader = BufReader::new(encoded.as_slice());
        let mut ide_out = Vec::new();

        forward(&mut reader, &mut ide_out, "LSP→IDE").await.unwrap();

        let received = read_frame(&mut BufReader::new(ide_out.as_slice())).await.unwrap().unwrap();
        assert_eq!(received, body);
    }

    #[tokio::test]
    async fn malformed_json_frame_is_skipped() {
        let bad = b"Content-Length: 3\r\n\r\nnot";
        let good = b"{\"ok\":true}";
        let mut input = bad.to_vec();
        write_frame(&mut input, good).await.unwrap();

        let mut reader = BufReader::new(input.as_slice());
        let mut writer = Vec::new();

        forward(&mut reader, &mut writer, "IDE→LSP").await.unwrap();

        let mut out = BufReader::new(writer.as_slice());
        assert_eq!(read_frame(&mut out).await.unwrap().unwrap(), good);
        assert!(read_frame(&mut out).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn eof_on_ide_input_terminates_run() {
        // Both sides return EOF immediately; run must complete without hanging.
        run(
            BufReader::new(&[][..]),
            Vec::new(),
            Vec::new(),
            BufReader::new(&[][..]),
        )
        .await
        .unwrap();
    }
}
