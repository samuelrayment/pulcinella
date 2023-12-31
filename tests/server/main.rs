cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        mod server;
        mod client_integration;
        mod helpers;
        mod server_safety;
    }
}
