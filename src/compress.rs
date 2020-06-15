use crate::Error;

use std::io::Write;
#[cfg(feature = "enable-tokio")]
use tokio::sync::mpsc;
#[cfg(feature = "enable-tokio")]
use tracing::*;
use zstd::stream::Encoder;

type VecEncoder = Encoder<Vec<u8>>;

pub struct ProstEncoder {
    inner: VecEncoder,
    protobuf: Vec<u8>,
}

impl ProstEncoder {
    pub fn new(level: i32) -> Result<Self, Error> {
        let buf = vec![];
        let inner = VecEncoder::new(buf, level).map_err(Error::Zstd)?;
        let protobuf = Self::new_protobuf();
        Ok(Self { inner, protobuf })
    }

    pub fn write<M: prost::Message>(&mut self, message: &M) -> Result<usize, Error> {
        let mut buf = vec![];
        message.encode(&mut buf).unwrap();
        message.encode_length_delimited(&mut self.protobuf)?;
        if self.protobuf.len() >= VecEncoder::recommended_input_size() {
            self.flush()?;
        }
        Ok(message.encoded_len())
    }

    pub fn compressed_len(&self) -> usize {
        self.inner.get_ref().len()
    }

    pub fn finish(mut self) -> Result<Vec<u8>, Error> {
        if self.protobuf.len() != 0 {
            self.flush()?;
        }
        self.inner.finish().map_err(Error::Zstd)
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.inner.write_all(&self.protobuf).map_err(Error::Zstd)?;
        self.protobuf = Self::new_protobuf();
        Ok(())
    }

    fn new_protobuf() -> Vec<u8> {
        Vec::with_capacity(VecEncoder::recommended_input_size())
    }
}

#[cfg(feature = "enable-tokio")]
type Rx<M> = mpsc::Receiver<M>;
#[cfg(feature = "enable-tokio")]
type Tx = mpsc::Sender<Vec<u8>>;

#[cfg(feature = "enable-tokio")]
pub struct Compressor<M: prost::Message> {
    rx: Rx<M>,
    tx: Tx,
    chunk_size: usize,
    level: i32,
}

#[cfg(feature = "enable-tokio")]
impl<M: prost::Message> Compressor<M> {
    pub fn new(rx: Rx<M>, tx: Tx, chunk_size: usize, level: i32) -> Self {
        Self {
            rx,
            tx,
            chunk_size,
            level,
        }
    }

    pub async fn compress(mut self) -> Result<(), Error> {
        trace!("compress started");
        let mut encoder = ProstEncoder::new(self.level)?;
        let mut wrote_len = 0;

        while let Some(update) = self.rx.recv().await {
            wrote_len += encoder.write(&update)?;
            let compressed_len = encoder.compressed_len();

            trace!(
                wrote_len,
                compressed_len,
                recommended_input_size = VecEncoder::recommended_input_size(),
                "encoded update",
            );

            if compressed_len >= self.chunk_size {
                let compressed = encoder.finish()?;
                trace!(compressed_len, "sending chunk",);
                self.tx.send(compressed).await?;

                encoder = ProstEncoder::new(self.level)?;
            }
        }

        let compressed = encoder.finish()?;
        let compressed_len = compressed.len();
        if compressed.len() > 0 {
            trace!(compressed_len, "sending final chunk");
            self.tx.send(compressed).await?;
        }

        trace!("compress ended");

        Ok(())
    }
}
