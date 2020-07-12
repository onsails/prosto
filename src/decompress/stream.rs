use crate::decompress::*;
use crate::Error;
use crate::StreamError;
use futures::prelude::*;
use futures::stream;

type Result<M> = std::result::Result<M, Error>;

pub struct Decompressor;

impl Decompressor {
    pub fn stream<
        M: prost::Message + std::default::Default,
        In: AsRef<[u8]>,
        S: Stream<Item = In>,
    >(
        in_stream: S,
    ) -> impl Stream<Item = Result<M>> {
        in_stream
            .map(|compressed| ProstDecoder::new_decompressed(compressed.as_ref()))
            .map_ok(stream::iter)
            .try_flatten()
            .into_stream()
    }

    pub fn try_stream<
        M: prost::Message + std::default::Default,
        In: AsRef<[u8]>,
        E,
        S: TryStream<Ok = In, Error = E>,
    >(
        in_stream: S,
    ) -> impl TryStream<Ok = M, Error = StreamError<E>> {
        in_stream
            .map_err(StreamError::Other)
            .and_then(|compressed| async move {
                let iter = ProstDecoder::new_decompressed(compressed.as_ref())
                    .map_err(StreamError::Prosto)?;
                Ok(iter)
            })
            .map_ok(stream::iter)
            .map_ok(|iter| {
                iter.map(|r| match r {
                    Ok(msg) => Ok(msg),
                    Err(e) => Err(StreamError::Prosto(e)),
                })
            })
            .try_flatten()
    }
}
