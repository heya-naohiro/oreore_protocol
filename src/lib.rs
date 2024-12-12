use bytes::{BufMut, BytesMut};
use futures_core::Stream;
use pin_project::pin_project;
use std::{pin::Pin, task::Poll};
mod parser;

type OreProtocolStreamResult = Result<parser::OreProtocol, std::io::Error>;
type ByteStream = Result<bytes::Bytes, std::io::Error>;

#[pin_project]
struct OreStream<S>
where
    S: Stream<Item = ByteStream> + Unpin,
{
    #[pin]
    stream: S,
    buffer: BytesMut,
    protocol: parser::OreProtocol, // プロトコル解析状態を保持
}

impl<S> OreStream<S>
where
    S: Stream<Item = ByteStream> + Unpin,
{
    fn new(stream: S) -> Self {
        OreStream {
            stream,
            buffer: BytesMut::new(),
            protocol: parser::OreProtocol::new(),
        }
    }
    fn push_data(&mut self, data: Vec<u8>) {
        self.buffer.extend_from_slice(&data);
    }
}

impl<S> Stream for OreStream<S>
where
    S: Stream<Item = ByteStream> + Unpin,
{
    type Item = OreProtocolStreamResult;
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    this.buffer.extend(&chunk);
                }
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Some(Err(error))),
                Poll::Ready(None) => {
                    if this.buffer.is_empty() {
                        return Poll::Ready(None);
                    }
                }
                Poll::Pending => return Poll::Pending,
            }
            match this.protocol.state {
                parser::ProtocolState::WaitHeader => {
                    if let Err(e) = this.protocol.parse_fixed_header(this.buffer) {
                        // **
                        return Poll::Pending;
                    }
                }
                parser::ProtocolState::WaitPayload => {
                    if let Err(e) = this.protocol.parse_payload(this.buffer) {
                        return Poll::Pending;
                    }
                    // 奪って入れ替える、もともとにはDefaultを突っ込まれる
                    // https://scrapbox.io/koki/%E6%A7%8B%E9%80%A0%E4%BD%93%E3%81%AE_&mut_%E5%8F%82%E7%85%A7%E3%81%8B%E3%82%89%E3%83%95%E3%82%A3%E3%83%BC%E3%83%AB%E3%83%89%E3%81%AE%E6%89%80%E6%9C%89%E6%A8%A9%E3%82%92%E5%A5%AA%E3%81%86
                    let completed_protocol = std::mem::take(this.protocol);
                    *this.protocol = parser::OreProtocol::new();
                    return Poll::Ready(Some(Ok(completed_protocol)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::{stream, StreamExt};
    use std::io;
    fn mock_stream() -> impl futures::Stream<Item = ByteStream> {
        stream::iter(vec![
            Ok(Bytes::from(vec![0x00, 0x0b])),
            Ok(Bytes::from("hello world")),
        ])
    }

    #[tokio::test]
    async fn test_ore_stream1() {
        let mock_stream = mock_stream();
        let mut ore_stream = OreStream::new(mock_stream);
        let mut results = vec![];

        while let Some(result) = ore_stream.next().await {
            match result {
                Ok(protocol) => results.push(protocol),
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                    break;
                }
            }
        }
        println!("{:?}", results)
    }
}
