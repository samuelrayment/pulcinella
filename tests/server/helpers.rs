use std::net::SocketAddr;

use wasm_test_server::server::{bind_socket, run, Mode};

pub(crate) async fn start_server(mode: Mode) -> u16 {
    let (port, listener) = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let server = run(listener, mode);
    let _ = tokio::spawn(server);
    port
}
