
use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::net::SocketAddr;

#[derive(Serialize)]
struct Slide {
    id: u32,
    title: String,
    content: String,
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/slides", get(get_slides));

    let address = SocketAddr::from(([127, 0, 0, 1], 3001));

    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("Failed to bind server");

    println!("Octo Slides API running at http://{}", address);

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}

async fn health_check() -> &'static str {
    "Octo Slides API is healthy"
}

async fn get_slides() -> Json<Vec<Slide>> {
    Json(vec![
        Slide {
            id: 1,
            title: "Introduction to Cybersecurity".to_string(),
            content: "Understanding digital threats and protection.".to_string(),
        },
        Slide {
            id: 2,
            title: "Common Cyber Threats".to_string(),
            content: "Phishing, malware, and ransomware.".to_string(),
        },
    ])
}