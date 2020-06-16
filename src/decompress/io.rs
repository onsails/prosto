use crate::decompress::*;
use crate::Error;
use tokio::sync::mpsc;
use tracing::*;

type Rx = mpsc::Receiver<Vec<u8>>;
type Tx<M> = mpsc::Sender<M>;

pub struct Decompressor<M: prost::Message> {
    rx: Rx,
    tx: Tx<M>,
}

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
