use fake::{Fake, Faker};

use pulcinella::{
    client::{Client, ClientError, Method},
    server::Mode,
};

use crate::helpers::{start_server, ServerPorts};

#[tokio::test]
async fn should_respond_with_404_when_no_mocks_specified() {
    let Dsl {
        control: mock_client,
        reqwest_client: client,
        ..
    } = setup_server().await;
    let response = client
        .get(&format!("{}/non-existent-path", mock_client.url()))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn should_respond_with_200_for_matched_path() {
    let path = format!("/{}", Faker.fake::<String>());
    let Dsl {
        control: mock_client,
        reqwest_client: client,
        ..
    } = setup_server().await;
    mock_client
        .when(|when| when.path(&path))
        .then(|then| then.status(200))
        .send()
        .await
        .expect("Failed to install mock");

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
    assert_ne!(
        server_path, client_path,
        "Server path should not match client path"
    );
    let Dsl {
        control: mock_client,
        reqwest_client: client,
        ..
    } = setup_server().await;
    mock_client
        .when(|when| when.path(&server_path))
        .then(|then| then.status(200))
        .send()
        .await
        .expect("Failed to install mock");

    let client = client
        .get(&format!("{}{}", mock_client.url(), client_path))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(client.status(), 404);
}

#[tokio::test]
async fn should_respond_with_another_status_code_for_matched_path() {
    let server_path = format!("/{}", Faker.fake::<String>());
    let Dsl {
        control: mock_client,
        reqwest_client: client,
        ..
    } = setup_server().await;

    mock_client
        .when(|when| when.path(&server_path))
        .then(|then| then.status(201))
        .send()
        .await
        .expect("Failed to install mock");

    let client = client
        .get(&format!("{}{}", mock_client.url(), server_path))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(client.status(), 201);
}

#[tokio::test]
async fn should_respond_with_404_for_matched_path_and_unmatched_form_data() {
    let form_name: String = Faker.fake();
    let form_name2: String = Faker.fake();
    let form_value: String = Faker.fake();
    let form_value2: String = Faker.fake();

    let server_path = format!("/{}", Faker.fake::<String>());
    let Dsl {
        control: mock_client,
        reqwest_client: client,
        ..
    } = setup_server().await;

    mock_client
        .when(|when| {
            when.path(&server_path)
                .form_data(&form_name, &form_value)
                .form_data(&form_name2, &form_value2)
        })
        .then(|then| then.status(200))
        .send()
        .await
        .expect("Failed to install mock");
    let client = client
        .get(&format!("{}{}", mock_client.url(), server_path))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(client.status(), 404);
}

#[tokio::test]
async fn should_respond_with_404_for_unmatched_method() {
    let server_path = format!("/{}", Faker.fake::<String>());
    let Dsl {
        control: mock_client,
        reqwest_client: client,
        ..
    } = setup_server().await;

    mock_client
        .when(|when| {
            when.path(&server_path)
                .method(Method::DELETE)
        })
        .then(|then| then.status(200))
        .send()
        .await
        .expect("Failed to install mock");
    let client = client
        .get(&format!("{}{}", mock_client.url(), server_path))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(client.status(), 404);
}


#[tokio::test]
async fn should_respond_with_the_most_specific_mock() {
    let form_name: String = Faker.fake();
    let form_name2: String = Faker.fake();
    let form_value: String = Faker.fake();
    let form_value2: String = Faker.fake();

    let server_path = format!("/{}", Faker.fake::<String>());
    let Dsl {
        control: mock_client,
        reqwest_client: client,
        ..
    } = setup_server().await;

    mock_client
        .when(|when| when.path(&server_path))
        .then(|then| then.status(200))
        .send()
        .await
        .expect("Failed to install mock");
    // more specific mock
    mock_client
        .when(|when| {
            when.path(&server_path)
                .form_data(&form_name, &form_value)
                .form_data(&form_name2, &form_value2)
        })
        .then(|then| then.status(201))
        .send()
        .await
        .expect("Failed to install mock");

    let client = client
        .post(&format!("{}{}", mock_client.url(), server_path))
        .form(&[(form_name, form_value), (form_name2, form_value2)])
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(client.status(), 201);
}

#[tokio::test]
async fn should_respond_with_headers() {
    let header_name = Faker.fake::<String>();
    let header_value = Faker.fake::<String>();
    let server_path = format!("/{}", Faker.fake::<String>());
    let Dsl {
        control: mock_client,
        reqwest_client: client,
        ..
    } = setup_server().await;

    mock_client
        .when(|when| when.path(&server_path))
        .then(|then| then.status(200).header(&header_name, &header_value))
        .send()
        .await
        .expect("Failed to install mock");

    let client = client
        .get(&format!("{}{}", mock_client.url(), server_path))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(client.status(), 200);
    assert_header(client, &header_name, &header_value);
}

#[tokio::test]
async fn should_respond_with_a_body() {
    let body = Faker.fake::<String>();
    let server_path = format!("/{}", Faker.fake::<String>());
    let Dsl {
        control: mock_client,
        reqwest_client: client,
        ..
    } = setup_server().await;

    mock_client
        .when(|when| when.path(&server_path))
        .then(|then| then.status(200).body(body.clone()))
        .send()
        .await
        .expect("Failed to install mock");

    let client = client
        .get(&format!("{}{}", mock_client.url(), server_path))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(client.status(), 200);
    assert_eq!(body, client.text().await.unwrap());
}

#[tokio::test]
async fn should_fail_if_reusing_an_old_client() {
    let Dsl {
        control: old_mock_client,
        server_ports,
        ..
    } = setup_server().await;
    let _new_mock_client = Client::new(&format!("http://localhost:{}", server_ports.control_plane))
        .await
        .expect("mock client failed to start");

    let result = old_mock_client
        .when(|when| when.path("/"))
        .then(|then| then.status(200))
        .send()
        .await;
    assert_eq!(result, Err(ClientError::InstanceNoLongerValid));
}

#[tokio::test]
async fn should_forget_old_mocks_when_new_client() {
    let Dsl {
        control: old_mock_client,
        reqwest_client: client,
        server_ports,
        ..
    } = setup_server().await;
    let server_path = format!("/{}", Faker.fake::<String>());
    old_mock_client
        .when(|when| when.path(&server_path))
        .then(|then| then.status(200))
        .send()
        .await
        .expect("Failed to install mock");

    let _new_mock_client = Client::new(&format!("http://localhost:{}", server_ports.control_plane))
        .await
        .expect("mock client failed to start");

    let response = client
        .get(&format!("{}{}", old_mock_client.url(), server_path))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(response.status(), 404);
}

fn assert_header(
    response: reqwest::Response,
    header_name: impl AsRef<str>,
    header_value: impl AsRef<str>,
) {
    assert_eq!(
        header_value.as_ref().as_bytes(),
        response
            .headers()
            .get(header_name.as_ref())
            .expect("Expected header to be present")
            .as_bytes(),
    );
}

struct Dsl {
    control: Client,
    reqwest_client: reqwest::Client,
    server_ports: ServerPorts,
}

async fn setup_server() -> Dsl {
    let server_ports = start_server(Mode::Mock).await;
    let mock_client = Client::new(&format!("http://localhost:{}", server_ports.control_plane))
        .await
        .expect("mock client failed to start");
    let client = reqwest::Client::new();
    Dsl {
        control: mock_client,
        reqwest_client: client,
        server_ports,
    }
}
