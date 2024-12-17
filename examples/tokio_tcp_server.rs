use bytes::Bytes;
use futures::{StreamExt, TryStreamExt};
use tokio_util::codec::{BytesCodec, FramedRead};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server listening on 127.0.0.1:8080");

    loop {
        let (socket, _addr) = listener.accept().await?;
        tokio::spawn(async move {
            let framed = FramedRead::new(socket, BytesCodec::new())
                .map_ok(|bytes_mut| Bytes::from(bytes_mut));
            let mut ore_stream = ore_protocol::OreStream::new(framed);

            while let Some(item) = ore_stream.next().await {
                match item {
                    Ok(protocol) => {
                        println!("Parsed protocol: {:?}", protocol);
                    }
                    Err(e) => {
                        eprintln!("Error: {:?}", e);
                        break;
                    }
                }
            }
        });
    }
}
