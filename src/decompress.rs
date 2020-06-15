use crate::Error;
use std::io::Cursor;
#[cfg(feature = "enable-tokio")]
use tokio::sync::mpsc;
#[cfg(feature = "enable-tokio")]
use tracing::*;

pub struct ProstDecoder<M: prost::Message> {
    cursor: Cursor<Vec<u8>>,
    len: u64,
    __phantom: std::marker::PhantomData<M>,
}

impl<M: prost::Message> ProstDecoder<M> {
    pub fn new_decompressed(compressed: &[u8]) -> Result<Self, Error> {
        let decompressed = zstd::decode_all(compressed).map_err(Error::Zstd)?;
        let len = decompressed.len() as u64;
        let cursor = Cursor::new(decompressed);
        Ok(Self {
            cursor,
            len,
            __phantom: std::marker::PhantomData,
        })
    }
}

#[cfg(feature = "enable-tokio")]
type Rx = mpsc::Receiver<Vec<u8>>;
#[cfg(feature = "enable-tokio")]
type Tx<M> = mpsc::Sender<M>;

#[cfg(feature = "enable-tokio")]
pub struct Decompressor<M: prost::Message> {
    rx: Rx,
    tx: Tx<M>,
}

impl<M: prost::Message + std::default::Default> std::iter::Iterator for ProstDecoder<M> {
    type Item = Result<M, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor.position() != self.len {
            let result = M::decode_length_delimited(&mut self.cursor).map_err(Error::from);
            Some(result)
        } else {
            None
        }
    }
}

#[cfg(feature = "enable-tokio")]
impl<M: prost::Message + std::default::Default> Decompressor<M> {
    pub fn new(rx: Rx, tx: Tx<M>) -> Self {
        Self { rx, tx }
    }

    pub async fn decompress(mut self) -> Result<(), Error> {
        trace!("decompress started");
        while let Some(compressed) = self.rx.recv().await {
            let decoder = ProstDecoder::new_decompressed(compressed.as_slice())?;
            for update in decoder {
                let update = update?;
                self.tx
                    .send(update)
                    .await
                    .map_err(|_| Error::DecompressorSend)?;
            }
        }
        trace!("decompress ended");

        Ok(())
    }
}
