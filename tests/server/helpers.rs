use std::net::SocketAddr;

use wasm_test_server::server::{bind_socket, run_controlplane, Mode, run_mock, SequentialState};

pub(crate) struct ServerPorts {
    pub(crate) control_plane: u16,
    pub(crate) mock: u16,
}

pub(crate) async fn start_server(mode: Mode) -> ServerPorts {
    let control_plane = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let mock = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();

    let state = SequentialState::new(mock.port);
    let control_plane_server = run_controlplane(control_plane.listener, state.clone());
    tokio::spawn(control_plane_server);

    let proxy_server = run_mock(mock.listener, state, mode);
    tokio::spawn(proxy_server);

    ServerPorts {
        control_plane: control_plane.port,
        mock: mock.port,
    }
}
