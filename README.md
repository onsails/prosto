# Compress prost! messages with zstd, async streams support

[![docs](https://docs.rs/prosto/badge.svg)](https://docs.rs/prosto)

## Simple compress/decompress

```rust
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
```

## Async streams support

`enable-async` Cargo feature (enabled by default) exposes `Compressor` and `Decompressor` structs:

* `Compressor::build_stream` converts a stream of prost! messages to a stream of bytes;
* `Decompressor::stream` converts a stream of compressed bytes to a stream of prost! messages.

Despite this example utilizes tokio channels, this crate does not depend on tokio, it's just used in tests.

```rust
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
```
