use crate::decompress::*;
use crate::Error;
use futures::prelude::*;
use futures::stream;

type Result<M> = std::result::Result<M, Error>;

pub struct Decompressor;

impl Decompressor {
    pub fn new<M: prost::Message + std::default::Default, In: AsRef<[u8]>, S: Stream<Item = In>>(
        in_stream: S,
    ) -> impl Stream<Item = Result<M>> {
        in_stream
            .map(|compressed| ProstDecoder::new_decompressed(compressed.as_ref()))
            .map_ok(stream::iter)
            .try_flatten()
            .into_stream()
    }
}
