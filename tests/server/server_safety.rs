use reqwest::Method;
use wasm_test_server::server::Mode;

use crate::helpers::{start_server, ServerPorts};

#[tokio::test]
async fn should_handle_no_body_passed_to_control_plane() {
    let dsl = setup_server().await;

    let response = dsl
        .client
        .post(&format!(
            "http://localhost:{}/",
            dsl.server_ports.control_plane
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(400, response.status());
}

#[tokio::test]
async fn should_handle_cors_for_control_port() {
    let dsl = setup_server().await;

    let response = dsl
        .client
        .request(Method::OPTIONS, &format!(
            "http://localhost:{}/",
            dsl.server_ports.control_plane
        ))
        .send()
        .await
        .expect("Failed to send request");
    let headers = response.headers();

    assert_eq!(200, response.status());
    assert_eq!("*", headers.get("access-control-allow-origin").unwrap());
}

#[tokio::test]
async fn should_handle_cors_for_mock_port() {
    let dsl = setup_server().await;

    let response = dsl
        .client
        .request(Method::OPTIONS, &format!(
            "http://localhost:{}/",
            dsl.server_ports.mock
        ))
        .send()
        .await
        .expect("Failed to send request");
    let headers = response.headers();

    assert_eq!(200, response.status());
    assert_eq!("*", headers.get("access-control-allow-origin").unwrap());
}

struct Dsl {
    client: reqwest::Client,
    server_ports: ServerPorts,
}

async fn setup_server() -> Dsl {
    let server_ports = start_server(Mode::Mock).await;
    let client = reqwest::Client::new();
    Dsl {
        client,
        server_ports,
    }
}
