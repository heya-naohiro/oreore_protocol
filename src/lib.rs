use bytes::BytesMut;
use futures_core::ready;
use futures_core::Stream;
use pin_project::pin_project;
use std::{pin::Pin, task::Poll};
mod decoder;

type OreProtocolStreamResult = Result<decoder::OreProtocol, std::io::Error>;
type ByteStream = Result<bytes::Bytes, std::io::Error>;

#[pin_project]
pub struct OreStream<S>
where
    S: Stream<Item = ByteStream> + Unpin,
{
    #[pin]
    stream: S,
    buffer: BytesMut,
    protocol: decoder::OreProtocol, // プロトコル解析状態を保持
}

impl<S> OreStream<S>
where
    S: Stream<Item = ByteStream> + Unpin,
{
    pub fn new(stream: S) -> Self {
        OreStream {
            stream,
            buffer: BytesMut::new(),
            protocol: decoder::OreProtocol::new(),
        }
    }
}

impl<S> Stream for OreStream<S>
where
    S: Stream<Item = ByteStream> + Unpin,
{
    type Item = OreProtocolStreamResult;
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.as_mut().project();
        //dbg!(&this.protocol);
        if !this.buffer.is_empty() {
            match this.protocol.state {
                decoder::ProtocolState::WaitHeader => {
                    if let Err(_e) = this.protocol.decode_fixed_header(this.buffer) {
                        // Insufficient
                        // go to waiting data...
                    } else {
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                }
                decoder::ProtocolState::WaitPayload => {
                    if let Err(_e) = this.protocol.decode_payload(this.buffer) {
                        // Insufficient
                        // go to waiting data...
                    } else {
                        // 奪って入れ替える、もともとにはDefaultを突っ込まれる(resetされるのでこれでOK)
                        // https://scrapbox.io/koki/%E6%A7%8B%E9%80%A0%E4%BD%93%E3%81%AE_&mut_%E5%8F%82%E7%85%A7%E3%81%8B%E3%82%89%E3%83%95%E3%82%A3%E3%83%BC%E3%83%AB%E3%83%89%E3%81%AE%E6%89%80%E6%9C%89%E6%A8%A9%E3%82%92%E5%A5%AA%E3%81%86
                        let completed_protocol = std::mem::take(this.protocol);
                        *this.protocol = decoder::OreProtocol::new();
                        return Poll::Ready(Some(Ok(completed_protocol)));
                    }
                }
            }
        }

        match ready!(this.stream.poll_next(cx)) {
            Some(Ok(bytes)) => {
                this.buffer.extend_from_slice(&bytes);
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            Some(Err(e)) => return Poll::Ready(Some(Err(e))),
            None => {
                // [TODO] 終了する前にbufferの空かどうかのチェックが必要か状態遷移図を描いて検討する
                return Poll::Ready(None);
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;
    use bytes::Bytes;
    use futures::{stream, StreamExt};
    fn mock_stream(
        testdata: Vec<Result<Bytes, io::Error>>,
    ) -> impl futures::Stream<Item = ByteStream> {
        stream::iter(testdata)
    }

    #[tokio::test]
    async fn test_ore_stream_single() {
        let mock_stream = mock_stream(vec![
            Ok(Bytes::from(vec![0x00, 0x0b])),
            Ok(Bytes::from("hello world")),
        ]);
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
        assert_eq!(
            results[0].clone().payload.unwrap()[..],
            Bytes::from_static(b"hello world")
        );
    }

    #[tokio::test]
    async fn test_ore_stream_double() {
        let mock_stream = mock_stream(vec![
            Ok(Bytes::from(vec![0x00, 0x0b])),
            Ok(Bytes::from("hello world")),
            Ok(Bytes::from(vec![0x00, 0x0c])),
            Ok(Bytes::from("hello world2")),
        ]);
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
        assert_eq!(results.len(), 2);

        assert_eq!(
            results[0].clone().payload.unwrap()[..],
            Bytes::from_static(b"hello world")
        );
        assert_eq!(
            results[1].clone().payload.unwrap()[..],
            Bytes::from_static(b"hello world2")
        );
    }

    #[tokio::test]
    async fn test_ore_stream_insufficient() {
        let mock_stream = mock_stream(vec![
            Ok(Bytes::from(vec![0x00])),
            Ok(Bytes::from(vec![0x0b])),
            Ok(Bytes::from("hello")),
            Ok(Bytes::from(" world")),
            Ok(Bytes::from(vec![0x00])),
            Ok(Bytes::from(vec![0x0c])),
            Ok(Bytes::from("hello wo")),
            Ok(Bytes::from("rld2")),
        ]);
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
        assert_eq!(results.len(), 2);

        assert_eq!(
            results[0].clone().payload.unwrap()[..],
            Bytes::from_static(b"hello world")
        );
        assert_eq!(
            results[1].clone().payload.unwrap()[..],
            Bytes::from_static(b"hello world2")
        );
    }
}
