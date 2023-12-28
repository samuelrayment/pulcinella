use std::net::SocketAddr;

use fake::{Fake, Faker};
use wasm_test_server::{
    client::Client,
    server::{bind_socket, run_controlplane, Mode},
};

use crate::helpers::start_server;

#[tokio::test]
async fn should_respond_with_404_when_no_mocks_specified() {
    let server_ports = start_server(Mode::Mock).await;

    let client = reqwest::Client::new();
    let response = client
        .get(&format!("http://localhost:{}/non-existent-path", server_ports.mock))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn should_respond_with_200_for_matched_path() {
    let path = format!("/{}", Faker.fake::<String>());
    let server_ports = start_server(Mode::Mock).await;
    let mock_client = Client::new(&format!("http://localhost:{}", server_ports.control_plane))
        .await
        .expect("mock client failed to start");
    mock_client
        .when(|when| when.path(&path))
        .then(|then| then.status(200))
        .send()
        .await;

    let client = reqwest::Client::new();
    let client = client
        .get(&format!("{}{}", mock_client.url(), path))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(client.status(), 200);
}

#[tokio::test]
async fn should_respond_with_404_for_unmatched_path() {
    let server_path = format!("/{}", Faker.fake::<String>());
    let client_path = format!("/{}", Faker.fake::<String>());
    assert_ne!(server_path, client_path, "Server path should not match client path");
    let server_ports = start_server(Mode::Mock).await;
    let mock_client = Client::new(&format!("http://localhost:{}", server_ports.control_plane))
        .await
        .expect("mock client failed to start");
    mock_client
        .when(|when| when.path(&server_path))
        .then(|then| then.status(200))
        .send()
        .await;

    let client = reqwest::Client::new();
    let client = client
        .get(&format!("{}{}", mock_client.url(), client_path))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(client.status(), 404);
}
