pub mod compress;
pub mod decompress;
pub use compress::*;
pub use decompress::*;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Zstd failure: {:?}", 0)]
    Zstd(#[source] std::io::Error),
    #[error("IO error: {:?}", 0)]
    IO(#[source] std::io::Error),
    #[error("Failed to prost encode: {:?}", 0)]
    ProstEncode(
        #[from]
        #[source]
        prost::EncodeError,
    ),
    #[error("Failed to prost decode: {:?}", 0)]
    ProstDecode(
        #[from]
        #[source]
        prost::DecodeError,
    ),
}

#[cfg(feature = "enable-async")]
pub enum StreamError<E> {
    Prosto(Error),
    Other(E),
}

#[cfg(feature = "enable-async")]
#[cfg(test)]
#[macro_use]
extern crate anyhow;

#[cfg(test)]
mod tests {
    use super::compress::*;
    use super::decompress::*;
    #[cfg(feature = "enable-async")]
    use futures::prelude::*;
    use proptest::prelude::*;
    #[cfg(feature = "enable-async")]
    use tokio::runtime::Runtime;
    #[cfg(feature = "enable-async")]
    use tokio::sync::mpsc;

    mod proto {
        tonic::include_proto!("dummy");
    }

    #[test]
    fn roundtrip_coders_simple() {
        do_roundtrip_coders(5, dummy_dummies(3));
        do_roundtrip_coders(0, dummy_dummies(10000));
    }

    #[cfg(feature = "enable-async")]
    #[test]
    fn roundtrip_channels_simple() {
        do_roundtrip_channels(0, 0, dummy_dummies(3));
        do_roundtrip_channels(1024, 5, dummy_dummies(3));
        do_roundtrip_channels(1000000, 5, dummy_dummies(3));
        do_roundtrip_channels(0, 0, dummy_dummies(1993));
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 20,
            timeout: 60_000,
            .. ProptestConfig::default()
        })]

        #[test]
        fn roundtrip_coders_prop(level in (-5..22), dummies in arb_dummies()) {
            do_roundtrip_coders(level, dummies);
        }

        #[cfg(feature = "enable-async")]
        #[test]
        fn roundtrip_channels_prop(chunk_size in (0usize..256*1024), level in (-5..22), dummies in arb_dummies()) {
            do_roundtrip_channels(chunk_size, level, dummies);
        }
    }

    fn do_roundtrip_coders(level: i32, dummies: Vec<proto::Dummy>) {
        tracing_subscriber::fmt::try_init().ok();

        let writer = vec![];
        let mut encoder = ProstEncoder::new(writer, level).unwrap();
        for dummy in &dummies {
            encoder.write(dummy).unwrap();
        }
        let compressed = encoder.finish().unwrap();

        let mut decoder = ProstDecoder::<proto::Dummy>::new_decompressed(&compressed[..]).unwrap();

        let mut i: usize = 0;
        while let Some(dummy) = decoder.next() {
            let dummy = dummy.unwrap();
            assert_eq!(&dummy, dummies.get(i).unwrap());
            i += 1;
        }

        assert_eq!(dummies.len(), i);
    }

    #[cfg(feature = "enable-async")]
    fn do_roundtrip_channels(chunk_size: usize, level: i32, dummies: Vec<proto::Dummy>) {
        tracing_subscriber::fmt::try_init().ok();

        let mut rt = Runtime::new().unwrap();

        // Dummy source ~> Compressor
        let (mut source, dummy_rx) = mpsc::channel::<proto::Dummy>(dummies.len());
        // Compressor ~> Decompressor
        let (compressed_tx, compressed_rx) = mpsc::channel::<Vec<u8>>(dummies.len());
        // Decompressor ~> Dummy sink
        let (dummy_tx, mut sink) = mpsc::channel::<proto::Dummy>(dummies.len());

        let compressor = Compressor::build_stream(dummy_rx, level, chunk_size).unwrap();
        let decompressor = Decompressor::stream(compressed_rx);

        rt.block_on(async move {
            let compress_task = tokio::task::spawn(
                compressor
                    .map_err(anyhow::Error::new)
                    .try_fold(compressed_tx, |mut ctx, compressed| async {
                        ctx.send(compressed)
                            .await
                            .map_err(|_| anyhow!("Failed to send compressed"))?;
                        Ok(ctx)
                    })
                    .map_ok(|_| ()),
            );
            let decompress_task = tokio::task::spawn(
                decompressor
                    .map_err(anyhow::Error::new)
                    .try_fold(dummy_tx, |mut utx, message| async {
                        utx.send(message)
                            .await
                            .map_err(|_| anyhow!("Failed to send decompressed"))?;
                        Ok(utx)
                    })
                    .map_ok(|_| ()),
            );

            for dummy in &dummies {
                source
                    .send(dummy.clone())
                    .await
                    .map_err(|_| anyhow!("Failed to send to source"))
                    .unwrap();
            }

            std::mem::drop(source);

            let mut i: usize = 0;
            while let Some(dummy) = sink.recv().await {
                assert_eq!(&dummy, dummies.get(i).unwrap());
                i += 1;
            }

            let (compress, decompress) =
                futures::try_join!(compress_task, decompress_task).unwrap();
            compress.unwrap();
            decompress.unwrap();
            assert_eq!(dummies.len(), i);
        });
    }

    fn dummy_dummies(count: usize) -> Vec<proto::Dummy> {
        let mut dummies = vec![];
        for id in 0..count {
            let dummy = proto::Dummy {
                id: id as u64,
                smth: (0..id as u8).collect(),
            };
            dummies.push(dummy);
        }
        dummies
    }

    prop_compose! {
        fn arb_dummy()
                    (id in any::<u64>(),
                     smth in prop::collection::vec(any::<u8>(), 0..1024)) -> proto::Dummy {
            proto::Dummy { id, smth }
        }
    }

    prop_compose! {
        fn arb_dummies()(dummies in prop::collection::vec(arb_dummy(), 0..10_000)) -> Vec<proto::Dummy> {
            dummies
        }
    }
}
