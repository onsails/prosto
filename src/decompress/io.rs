use crate::decompress::*;
use crate::Error;
use tokio::sync::mpsc;
use tracing::*;

type Rx<In> = mpsc::Receiver<In>;
type Tx<M> = mpsc::Sender<M>;

pub struct Decompressor<M: prost::Message, In: AsRef<[u8]>> {
    rx: Rx<In>,
    tx: Tx<M>,
}

impl<M: prost::Message + std::default::Default, In: AsRef<[u8]>> Decompressor<M, In> {
    pub fn new(rx: Rx<In>, tx: Tx<M>) -> Self {
        Self { rx, tx }
    }

    pub async fn decompress(mut self) -> Result<(), Error> {
        trace!("decompress started");
        while let Some(compressed) = self.rx.recv().await {
            let decoder = ProstDecoder::new_decompressed(compressed.as_ref())?;
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
