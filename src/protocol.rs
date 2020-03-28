use async_std::fs::File as AsyncFile;
use crypto::digest::Digest;
use crypto::sha1::Sha1;
use futures::{io as asyncio, prelude::*};
use libp2p::core::{InboundUpgrade, OutboundUpgrade, UpgradeInfo};
use std::fs::File;
use std::io::{BufReader, Read};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{io, iter, pin::Pin};

const CHUNK_SIZE: usize = 4096;

pub struct FileToSend {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug)]
pub enum ProtocolEvent {
    Received {
        name: String,
        path: String,
        hash: String,
        size_bytes: usize,
    },
    Sent,
}

#[derive(Clone, Debug, Default)]
pub struct TransferPayload {
    pub name: String,
    pub path: String,
    pub hash: String,
    pub size_bytes: usize,
}

impl TransferPayload {
    pub fn new(name: String, path: String, hash: String, size_bytes: usize) -> TransferPayload {
        TransferPayload {
            name,
            path,
            hash,
            size_bytes,
        }
    }

    pub fn check_file(&self) -> Result<(), io::Error> {
        let mut contents = vec![];
        let mut file = BufReader::new(File::open(&self.path)?);
        file.read_to_end(&mut contents).expect("Cannot read file");
        let hash_from_disk = hash_contents(&mut contents);

        if hash_from_disk != self.hash {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "File corrupted!",
            ))
        } else {
            Ok(())
        }
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

fn add_row(value: &str) -> Vec<u8> {
    format!("{}\n", value).into_bytes()
}

fn hash_contents(contents: &Vec<u8>) -> String {
    let mut hasher = Sha1::new();
    hasher.input(&contents);
    hasher.result_str()
}

async fn read_socket(
    socket: impl AsyncRead + AsyncWrite + Send + Unpin,
) -> Result<TransferPayload, io::Error> {
    let mut reader = asyncio::BufReader::new(socket);
    let mut payloads: Vec<u8> = vec![];

    let mut name: String = "".into();
    let mut hash: String = "".into();
    reader.read_line(&mut name).await?;
    reader.read_line(&mut hash).await?;

    let (name, hash) = (name.trim(), hash.trim());
    println!("Name: {}, Hash: {}", name, hash);
    let now = SystemTime::now();
    let timestamp = now.duration_since(UNIX_EPOCH).expect("Time failed");

    let path = format!("/tmp/files/{}_{}", timestamp.as_secs(), name);

    let mut file = asyncio::BufWriter::new(AsyncFile::create(&path).await?);
    let mut counter: usize = 0;
    loop {
        let mut buff = vec![0u8; CHUNK_SIZE];
        match reader.read(&mut buff).await {
            Ok(n) => {
                if n > 0 {
                    payloads.extend(&buff[..n]);
                    counter += n;
                    if payloads.len() > (1024 * 1024) {
                        file.write_all(&payloads)
                            .await
                            .expect("Writing file failed");
                        payloads.clear();
                    }
                } else {
                    file.write_all(&payloads)
                        .await
                        .expect("Writing file failed");
                    payloads.clear();
                    break;
                }
            }
            Err(e) => panic!("Failed reading the socket {:?}", e),
        }
    }

    let event = TransferPayload::new(
        name.to_string(),
        hash.to_string(),
        path.to_string(),
        counter,
    );

    println!("Name: {}, Read {:?} bytes", name, counter);
    Ok(event)
}

impl<TSocket> InboundUpgrade<TSocket> for TransferPayload
where
    TSocket: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = TransferPayload;
    type Error = asyncio::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

    fn upgrade_inbound(self, socket: TSocket, _: Self::Info) -> Self::Future {
        Box::pin(async move {
            println!("Upgrade inbound");
            let start = now();
            let event = read_socket(socket).await?;

            println!("Finished {:?} ms", start.elapsed().as_millis());
            Ok(event)
        })
    }
}

impl<TSocket> OutboundUpgrade<TSocket> for TransferPayload
where
    TSocket: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = ();
    type Error = asyncio::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

    fn upgrade_outbound(self, mut socket: TSocket, _: Self::Info) -> Self::Future {
        Box::pin(async move {
            println!("Upgrade outbound");
            let start = now();

            println!("Name: {:?}, Path: {:?}", self.name, self.path);

            let file = AsyncFile::open(self.path).await.expect("File missing");
            let mut buff = asyncio::BufReader::new(&file);
            let mut contents = vec![];
            buff.read_to_end(&mut contents)
                .await
                .expect("Cannot read file");

            let hash = hash_contents(&contents);
            let name = add_row(&self.name);
            let checksum = add_row(&hash);

            socket.write(&name).await.expect("Writing name failed");
            socket.write(&checksum).await?;
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
            hash: transfer.hash,
            size_bytes: transfer.size_bytes,
        }
    }
}
