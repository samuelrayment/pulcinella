use std::net::SocketAddr;

use wasm_test_server::server::{bind_socket, run, Mode};

pub(crate) struct ServerPorts {
    pub(crate) control_plane: u16,
    pub(crate) mock: u16,
}

pub(crate) async fn start_server(mode: Mode) -> u16 {
    let control_plane = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let mock = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let server = run(control_plane.listener, mode);
    let _ = tokio::spawn(server);
    control_plane.port
}
