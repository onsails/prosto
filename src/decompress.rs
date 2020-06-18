use crate::Error;
use std::io::Cursor;
#[cfg(feature = "enable-async")]
pub use stream::*;

#[cfg(feature = "enable-async")]
pub mod stream;

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
