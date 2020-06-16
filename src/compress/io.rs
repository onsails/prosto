use crate::Error;
use crate::compress::*;
use tokio::sync::mpsc;
use tracing::*;

type Rx<M> = mpsc::Receiver<M>;
type Tx = mpsc::Sender<Vec<u8>>;

pub struct Compressor<M: prost::Message> {
    rx: Rx<M>,
    tx: Tx,
    chunk_size: usize,
    level: i32,
}

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
        if !compressed.is_empty() {
            trace!(compressed_len, "sending final chunk");
            self.tx.send(compressed).await?;
        }

        trace!("compress ended");

        Ok(())
    }
}
