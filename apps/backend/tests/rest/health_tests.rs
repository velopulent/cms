use crate::common::TestServer;

#[tokio::test]
async fn health_probes_are_public_and_minimal() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let live = client
        .get(format!("{}/health/live", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(live.status(), 200);
    assert_eq!(
        live.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({ "status": "ok" })
    );

    let ready = client
        .get(format!("{}/health/ready", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(ready.status(), 200);
    assert_eq!(
        ready.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({ "status": "ready" })
    );
}
