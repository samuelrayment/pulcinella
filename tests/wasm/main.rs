cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        mod client_integration;
    }
}
