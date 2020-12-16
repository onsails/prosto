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
#[derive(thiserror::Error, Debug)]
pub enum StreamError<E: std::fmt::Debug> {
    #[error("Prosto: {:?}", 0)]
    Prosto(#[source] Error),
    #[error("Other: {:?}", 0)]
    Other(E),
}
