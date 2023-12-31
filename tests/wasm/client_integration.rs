use gloo_net::http::Request;
use wasm_bindgen_test::*;
use wasm_test_server::wasm_client::Client;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn should_return_200_for_mocked_path() {
    let path = format!("/matched-path");
    let mock_client = Client::new("http://localhost:3000")
        .await
        .expect("mock client failed to start");
    mock_client
        .when(|when| when.path(&path))
        .then(|then| then.status(200))
        .send()
        .await
        .expect("Failed to install mock");

    let response = Request::get(&format!("{}{}", mock_client.url(), path))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);
}
