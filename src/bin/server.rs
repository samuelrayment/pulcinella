use std::net::SocketAddr;

use wasm_test_server::server::{run_controlplane, bind_socket, Mode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    //let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    let control_plane = bind_socket(addr).await?;
    println!("Listening on http://127.0.0.1:{}/", control_plane.port);

    run_controlplane(control_plane.listener, Mode::Proxy).await
}
