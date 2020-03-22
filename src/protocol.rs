use async_std::fs::File;
use async_std::io;
use futures::{io as asyncio, prelude::*};
use libp2p::core::{InboundUpgrade, OutboundUpgrade, UpgradeInfo};
use std::iter;
use std::pin::Pin;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const CHUNK_SIZE: usize = 1024;

pub struct FileObject {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug)]
pub enum ProtocolEvent {
    Received { name: String, path: String },
    Sent,
}

#[derive(Clone, Debug, Default)]
pub struct TransferPayload {
    pub name: String,
    pub path: String,
}

impl TransferPayload {
    pub fn new(name: String, path: String) -> TransferPayload {
        TransferPayload { name, path }
    }
}

impl UpgradeInfo for TransferPayload {
    type Info = &'static str;
    type InfoIter = iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        std::iter::once("/transfer/1.0")
    }
}

fn now() -> Instant {
    Instant::now()
}

impl<TSocket> InboundUpgrade<TSocket> for TransferPayload
where
    TSocket: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = TransferPayload;
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

    fn upgrade_inbound(self, socket: TSocket, _: Self::Info) -> Self::Future {
        Box::pin(async move {
            println!("Upgrade inbound");
            let mut reader = asyncio::BufReader::new(socket);
            let start = now();
            let now = SystemTime::now();
            let timestamp = now.duration_since(UNIX_EPOCH).expect("Time failed");

            let name = "file".to_string();
            let path = format!("/tmp/files/{}_{}.flac", timestamp.as_secs(), name);
            let mut file =
                asyncio::BufWriter::new(File::create(&path).await.expect("Cannot create file"));

            let mut counter: usize = 0;
            let mut payloads: Vec<u8> = vec![];
            loop {
                let mut buff = vec![0u8; CHUNK_SIZE];
                match reader.read(&mut buff).await {
                    Ok(n) => {
                        if n > 0 {
                            payloads.extend(&buff[..n]);
                            counter += n;
                            if payloads.len() > (1024 * 128) {
                                file.write(&payloads).await.expect("Writing file failed");
                                payloads.clear();
                            }
                        } else {
                            file.write(&payloads).await.expect("Writing file failed");
                            payloads.clear();
                            break;
                        }
                    }
                    Err(e) => panic!("Failed reading the socket {:?}", e),
                }
            }
            println!("Finished {:?} ms", start.elapsed().as_millis());
            println!("Name: {}, Read {:?} bytes", name, counter);
            let event = TransferPayload::new(name, path);
            Ok(event)
        })
    }
}

impl<TSocket> OutboundUpgrade<TSocket> for TransferPayload
where
    TSocket: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = ();
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

    fn upgrade_outbound(self, mut socket: TSocket, _: Self::Info) -> Self::Future {
        Box::pin(async move {
            println!("Upgrade outbound");
            let start = now();
            let filename = "kot.mp4";
            let path = format!("/tmp/{}", filename);
            let mut file = asyncio::BufReader::new(File::open(path).await.expect("File missing"));
            let mut contents = vec![];
            file.read_to_end(&mut contents)
                .await
                .expect("Cannot read file");
            socket.write_all(&contents).await.expect("Writing failed");
            socket.close().await.expect("Failed to close socket");
            println!("Finished {:?} ms", start.elapsed().as_millis());
            Ok(())
        })
    }
}

impl From<()> for ProtocolEvent {
    fn from(_: ()) -> Self {
        ProtocolEvent::Sent
    }
}

impl From<TransferPayload> for ProtocolEvent {
    fn from(transfer: TransferPayload) -> Self {
        ProtocolEvent::Received {
            name: transfer.name,
            path: transfer.path,
        }
    }
}
