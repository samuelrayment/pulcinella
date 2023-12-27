use std::net::SocketAddr;

use wasm_test_server::server::{run, bind_socket, Mode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    //let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    let (port, listener) = bind_socket(addr).await?;
    println!("Listening on http://127.0.0.1:{}/", port);

    run(listener, Mode::Proxy).await
}
