use std::net::SocketAddr;

use tokio::join;
use wasm_test_server::server::{run_controlplane, bind_socket, Mode, SequentialState, run_mock};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    //let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let control_plane_addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let mock_addr = SocketAddr::from(([127, 0, 0, 1], 3001));

    let mock = bind_socket(mock_addr).await?;
    let control_plane = bind_socket(control_plane_addr).await?;
    let state = SequentialState::new(mock.port);

    println!("Control Plane on http://127.0.0.1:{}/", control_plane.port);
    println!("Mock on http://127.0.0.1:{}/", mock.port);

    let control_plane = run_controlplane(control_plane.listener, state.clone(), Mode::Proxy);
    let mock = run_mock(mock.listener, state, Mode::Proxy);
    let (cp_result, mock_result) = join!(control_plane, mock);

    cp_result.and(mock_result)
}
