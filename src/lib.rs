pub mod interchange;
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "client")]
pub(crate) mod shared_client;

#[cfg(all(feature = "client", not(target_arch = "wasm32")))]
pub mod client;
#[cfg(all(feature = "client", any(target_arch = "wasm32", rust_analyzer)))]
pub mod wasm_client;
#[cfg(all(feature = "client", any(target_arch = "wasm32", rust_analyzer)))]
pub use wasm_client as client;
pub(crate) mod network_client;
