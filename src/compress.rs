use crate::Error;
use std::io::Write;
#[cfg(feature = "enable-async")]
pub use stream::*;
use zstd::stream::Encoder;

#[cfg(feature = "enable-async")]
pub mod stream;

type VecEncoder<W> = Encoder<W>;

pub struct ProstEncoder<W: Write> {
    protobuf: Vec<u8>,
    inner: VecEncoder<W>,
}

impl<W: Write> ProstEncoder<W> {
    pub fn new(writer: W, level: i32) -> Result<Self, Error> {
        let inner = VecEncoder::new(writer, level).map_err(Error::Zstd)?;
        let protobuf = Self::new_protobuf();
        Ok(Self { inner, protobuf })
    }

    /// Finishes encoder. It has to be called before dropping ProstEncoder!
    #[tracing::instrument(skip(self))]
    pub fn finish(mut self) -> Result<W, Error> {
        tracing::trace!("finish");
        if !self.protobuf.is_empty() {
            self.flush()?;
        }
        self.inner.finish().map_err(Error::Zstd)
    }

    pub fn get_ref(&self) -> &W {
        self.inner.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut()
    }

    #[tracing::instrument(skip(self, message))]
    pub fn write<M: prost::Message>(&mut self, message: &M) -> Result<usize, Error> {
        let encoded_len = message.encoded_len();
        tracing::trace!(encoded_len, "writing message to the internal buffer");

        let mut buf = vec![];
        prost::encode_length_delimiter(message.encoded_len(), &mut buf).unwrap();
        message.encode_length_delimited(&mut self.protobuf)?;

        let recommended_input_size = VecEncoder::<W>::recommended_input_size();
        if self.protobuf.len() >= recommended_input_size {
            self.flush()?;
        }

        Ok(encoded_len)
    }

    /// Flushes internal serialized prorobuf buffer.
    /// Does not flush zstd Encoder!
    #[tracing::instrument(skip(self))]
    pub fn flush(&mut self) -> Result<(), Error> {
        tracing::trace!(len = self.protobuf.len(), "flush protobuf");
        // let size_delim = self.protobuf.len().to_le_bytes();
        // self.inner.write_all(&size_delim).map_err(Error::Zstd)?;
        self.inner.write_all(&self.protobuf).map_err(Error::Zstd)?;
        self.protobuf = Self::new_protobuf();
        Ok(())
    }

    /// Flushes zstd Encoder
    #[tracing::instrument(skip(self))]
    pub(crate) fn flush_inner(&mut self) -> Result<(), Error> {
        tracing::trace!("flush inner");
        self.inner.flush().map_err(Error::Zstd)?;
        Ok(())
    }

    fn new_protobuf() -> Vec<u8> {
        Vec::with_capacity(VecEncoder::<W>::recommended_input_size())
    }
}
