# Compress prost! messages with zstd, optional tokio channels support

[![docs](https://docs.rs/prosto/badge.svg)](https://docs.rs/prosto)

## Simple compress/decompress

```rust
fn do_roundtrip_coders(dummies: Vec<proto::Dummy>) {
    let mut encoder = ProstEncoder::new(0).unwrap();
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
```

## Tokio channels support

Cargo.toml:

```toml
prosto = { version = "0.1", features = ["enable-tokio"] }
```

```rust
fn do_roundtrip_channels(dummies: Vec<proto::Dummy>) {
    tracing_subscriber::fmt::try_init().ok();

    let mut rt = Runtime::new().unwrap();

    let (mut source, urx) = mpsc::channel::<proto::Dummy>(dummies.len());
    let (ctx, crx) = mpsc::channel::<Vec<u8>>(dummies.len());
    let (utx, mut sink) = mpsc::channel::<proto::Dummy>(dummies.len());

    let compressor = Compressor::new(urx, ctx, 256 * 1024, 5);
    let decompressor = Decompressor::new(crx, utx);

    rt.block_on(async move {
        tokio::task::spawn(compressor.compress());
        tokio::task::spawn(decompressor.decompress());

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
```
