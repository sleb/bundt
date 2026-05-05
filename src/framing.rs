use anyhow::{bail, Context, Result};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub async fn read_frame(reader: &mut (impl AsyncBufRead + Unpin)) -> Result<Option<Vec<u8>>> {
    let mut content_length: Option<usize> = None;
    let mut header_started = false;
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await.context("reading LSP header")?;

        if n == 0 {
            return if header_started {
                bail!("unexpected EOF mid-header")
            } else {
                Ok(None)
            };
        }

        header_started = true;
        let trimmed = line.trim_end_matches(['\r', '\n']);

        if trimmed.is_empty() {
            break; // end of headers
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length: ") {
            content_length = Some(value.parse::<usize>().with_context(|| {
                format!("invalid Content-Length value: {value:?}")
            })?);
        }
        // Other headers (e.g. Content-Type) are silently ignored
    }

    let len = content_length.context("missing Content-Length header")?;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await.context("reading LSP frame body")?;

    Ok(Some(body))
}

pub async fn write_frame(writer: &mut (impl AsyncWrite + Unpin), body: &[u8]) -> Result<()> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut frame = Vec::with_capacity(header.len() + body.len());
    frame.extend_from_slice(header.as_bytes());
    frame.extend_from_slice(body);
    writer.write_all(&frame).await.context("writing LSP frame")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn round_trip_single_frame() {
        let body = b"{\"jsonrpc\":\"2.0\"}";
        let mut encoded = Vec::new();
        write_frame(&mut encoded, body).await.unwrap();
        let result = read_frame(&mut BufReader::new(encoded.as_slice())).await.unwrap().unwrap();
        assert_eq!(result, body);
    }

    #[tokio::test]
    async fn round_trip_multiple_frames() {
        let frames: &[&[u8]] = &[b"{\"id\":1}", b"{\"id\":2}", b"{\"id\":3}"];
        let mut encoded = Vec::new();
        for body in frames {
            write_frame(&mut encoded, body).await.unwrap();
        }
        let mut reader = BufReader::new(encoded.as_slice());
        for expected in frames {
            let result = read_frame(&mut reader).await.unwrap().unwrap();
            assert_eq!(&result, expected);
        }
    }

    #[tokio::test]
    async fn clean_eof_returns_none() {
        let result = read_frame(&mut BufReader::new(&[][..])).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn eof_mid_header_returns_err() {
        // Content-Length present but no blank line or body — truncated stream
        let input = b"Content-Length: 5\r\n";
        assert!(read_frame(&mut BufReader::new(input.as_slice())).await.is_err());
    }

    #[tokio::test]
    async fn missing_content_length_returns_err() {
        let input = b"Content-Type: application/json\r\n\r\n";
        assert!(read_frame(&mut BufReader::new(input.as_slice())).await.is_err());
    }

    #[tokio::test]
    async fn non_numeric_content_length_returns_err() {
        let input = b"Content-Length: abc\r\n\r\n";
        assert!(read_frame(&mut BufReader::new(input.as_slice())).await.is_err());
    }

    #[tokio::test]
    async fn body_shorter_than_content_length_returns_err() {
        // Claims 100 bytes but only 2 are present
        let input = b"Content-Length: 100\r\n\r\n{}";
        assert!(read_frame(&mut BufReader::new(input.as_slice())).await.is_err());
    }
}
