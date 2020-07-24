use crate::compress::*;
use crate::Error;
use futures::prelude::*;
use futures::stream;
use std::io::Cursor;

type Result<M> = std::result::Result<M, Error>;

pub struct Compressor;

impl Compressor {
    pub fn build_stream<M: prost::Message, S: Stream<Item = M>>(
        in_stream: S,
        level: i32,
        chunk_size: usize,
    ) -> Result<impl Stream<Item = Result<Vec<u8>>>> {
        let mut buf = vec![];
        buf.reserve(chunk_size);
        let buf = Cursor::new(buf);
        let encoder = ProstEncoder::new(buf, level)?;

        let in_stream = in_stream.map(Some).chain(stream::once(async { None }));
        let stream = in_stream.scan(Some(encoder), move |encoder, message| {
            if let Some(message) = message {
                future::ready(Some(Self::write_produce(
                    encoder, &message, level, chunk_size,
                )))
            } else {
                future::ready(Some(Self::finish(encoder.take().unwrap())))
            }
        });
        let stream = stream
            .try_filter_map(|maybe| async { Ok(maybe) })
            .into_stream();
        Ok(stream)
    }

    #[tracing::instrument(level = "trace", skip(encoder, message))]
    fn write_produce<M: prost::Message>(
        encoder: &mut Option<ProstEncoder<Cursor<Vec<u8>>>>,
        message: &M,
        level: i32,
        chunk_size: usize,
    ) -> Result<Option<Vec<u8>>> {
        encoder.as_mut().unwrap().write(message)?;
        Self::maybe_produce(encoder, level, chunk_size)
    }

    #[tracing::instrument(level = "trace", skip(encoder))]
    fn maybe_produce(
        encoder: &mut Option<ProstEncoder<Cursor<Vec<u8>>>>,
        level: i32,
        chunk_size: usize,
    ) -> Result<Option<Vec<u8>>> {
        let compressed_len = encoder.as_ref().unwrap().get_ref().position() as usize;
        if compressed_len > chunk_size {
            tracing::trace!(compressed_len, "producing chunk");
            let compressed = encoder.take().unwrap().finish()?.into_inner();

            let buf = Vec::with_capacity(chunk_size);
            let buf = Cursor::new(buf);
            encoder.replace(ProstEncoder::new(buf, level)?);

            tracing::trace!(len = compressed.len(), "produced chunk");
            Ok(Some(compressed))
        } else {
            tracing::trace!(compressed_len, "not enough bytes for a chunk");
            Ok(None)
        }
    }

    #[tracing::instrument(level = "trace", skip(encoder))]
    fn finish(mut encoder: ProstEncoder<Cursor<Vec<u8>>>) -> Result<Option<Vec<u8>>> {
        encoder.flush().unwrap();
        tracing::trace!(position = encoder.get_ref().position(), "encoder finish");
        let cursor = encoder.finish()?;
        let position = cursor.position() as usize;
        tracing::trace!(position = position, "after encoder finish");
        if position != 0 {
            let mut compressed = cursor.into_inner();
            compressed.truncate(position);
            tracing::trace!(len = compressed.len(), "produced final chunk");
            Ok(Some(compressed))
        } else {
            Ok(None)
        }
    }
}
