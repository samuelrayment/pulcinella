use std::net::SocketAddr;

use wasm_test_server::bind_socket;

#[tokio::test]
async fn should_respond_with_404_for_non_existent_path() {
    let (port, listener) = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let server = wasm_test_server::run(listener);
    let _ = tokio::spawn(server);

    let client = reqwest::Client::new();
    let response = client
        .get(&format!("http://localhost:{}/non-existent-path", port))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 404);
}
