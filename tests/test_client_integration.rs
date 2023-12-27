use std::net::SocketAddr;

use wasm_test_server::{
    client::Client,
    server::{bind_socket, run, Mode},
};

#[tokio::test]
async fn should_respond_with_404_for_non_existent_path() {
    let port = start_server().await;

    let client = reqwest::Client::new();
    let response = client
        .get(&format!("http://localhost:{}/non-existent-path", port))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 404);
}

//#[tokio::test]
//async fn should_respond_with_200_for_matched_path() {
//    let port = start_server().await;
//    let mock_client = Client::new(&format!("http://localhost:{}", port))
//        .await
//        .expect("mock client failed to start");
//    mock_client
//        .when(|when| when.path("/matched-path"))
//        .then(|then| then.status(200))
//        .send()
//        .await;
//
//    let client = reqwest::Client::new();
//    let client = client
//        .get(&format!("{}/matched-path", mock_client.url()))
//        .send()
//        .await
//        .expect("Failed to send request");
//
//    assert_eq!(client.status(), 200);
//}

async fn start_server() -> u16 {
    let (port, listener) = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let server = run(listener, Mode::Mock);
    let _ = tokio::spawn(server);
    port
}
