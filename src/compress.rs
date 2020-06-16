use crate::Error;
#[cfg(feature = "enable-tokio")]
pub use io::*;
use std::io::Write;
use zstd::stream::Encoder;

#[cfg(feature = "enable-tokio")]
pub mod io;

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
        if !self.protobuf.is_empty() {
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

