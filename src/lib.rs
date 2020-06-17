#[cfg(feature = "enable-tokio")]
use tokio::sync::mpsc;

pub mod compress;
pub mod decompress;
pub use compress::*;
pub use decompress::*;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Zstd failure: {:?}", 0)]
    Zstd(#[source] std::io::Error),
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
    #[cfg(feature = "enable-tokio")]
    #[error("Failed to send compressed chunk: channel closed")]
    CompressorSend(
        #[from]
        #[source]
        mpsc::error::SendError<Vec<u8>>,
    ),
    #[cfg(feature = "enable-tokio")]
    #[error("Failed to send decompressed message: channel closed")]
    DecompressorSend,
}

#[cfg(test)]
mod tests {
    use super::compress::*;
    use super::decompress::*;
    #[cfg(feature = "enable-tokio")]
    use futures::prelude::*;
    use proptest::prelude::*;
    #[cfg(feature = "enable-tokio")]
    use tokio::runtime::Runtime;
    #[cfg(feature = "enable-tokio")]
    use tokio::sync::mpsc;

    mod proto {
        tonic::include_proto!("dummy");
    }

    #[test]
    fn roundtrip_coders_simple() {
        let dummies = dummy_dummies();
        do_roundtrip_coders(5, dummies);
    }

    #[cfg(feature = "enable-tokio")]
    #[test]
    fn roundtrip_channels_simple() {
        let dummies = dummy_dummies();
        do_roundtrip_channels(1024, 5, dummies);
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

        #[cfg(feature = "enable-tokio")]
        #[test]
        fn roundtrip_channels_prop(chunk_size in (0usize..256*1024), level in (-5..22), dummies in arb_dummies()) {
            do_roundtrip_channels(chunk_size, level, dummies);
        }
    }

    fn do_roundtrip_coders(level: i32, dummies: Vec<proto::Dummy>) {
        let mut encoder = ProstEncoder::new(level).unwrap();
        for dummy in &dummies {
            encoder.write(dummy).unwrap();
        }
        let compressed = encoder.finish().unwrap();

        let mut decoder =
            ProstDecoder::<proto::Dummy>::new_decompressed(compressed.as_slice()).unwrap();

        let mut i: usize = 0;
        while let Some(dummy) = decoder.next() {
            let dummy = dummy.unwrap();
            assert_eq!(&dummy, dummies.get(i).unwrap());
            i += 1;
        }

        assert_eq!(dummies.len(), i);
    }

    #[cfg(feature = "enable-tokio")]
    fn do_roundtrip_channels(chunk_size: usize, level: i32, dummies: Vec<proto::Dummy>) {
        tracing_subscriber::fmt::try_init().ok();

        let mut rt = Runtime::new().unwrap();

        let (mut source, urx) = mpsc::channel::<proto::Dummy>(dummies.len());
        let (ctx, crx) = mpsc::channel::<Vec<u8>>(dummies.len());
        let (utx, mut sink) = mpsc::channel::<proto::Dummy>(dummies.len());

        let compressor = Compressor::new(urx, ctx, chunk_size, level);
        let decompressor = Decompressor::new(crx);

        rt.block_on(async move {
            tokio::task::spawn(compressor.compress());
            tokio::task::spawn(decompressor.map_err(anyhow::Error::new).try_fold(
                utx,
                |mut utx, message| async {
                    utx.send(message).await.map_err(anyhow::Error::new)?;
                    Ok(utx)
                },
            ));

            for dummy in &dummies {
                source.send(dummy.clone()).await.unwrap();
            }

            std::mem::drop(source);

            let mut i: usize = 0;
            while let Some(dummy) = sink.recv().await {
                assert_eq!(&dummy, dummies.get(i).unwrap());
                i += 1;
            }

            assert_eq!(dummies.len(), i);
        });
    }

    fn dummy_dummies() -> Vec<proto::Dummy> {
        let mut dummies = vec![];
        for id in 0..3 {
            let dummy = proto::Dummy {
                id,
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
