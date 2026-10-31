
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
struct Slide {
    id: Uuid,
    title: String,
    content: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct Presentation {
    id: Uuid,
    title: String,
    slides: Vec<Slide>,
}

#[derive(Deserialize)]
struct CreatePresentation {
    title: String,
}

#[derive(Deserialize)]
struct AddSlide {
    title: String,
    content: String,
}

type Database = Arc<RwLock<HashMap<Uuid, Presentation>>>;

#[tokio::main]
async fn main() {
    let database: Database = Arc::new(RwLock::new(HashMap::new()));

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/presentations", get(list_presentations).post(create_presentation))
        .route("/api/presentations/{id}", get(get_presentation).delete(delete_presentation))
        .route("/api/presentations/{id}/slides", axum::routing::post(add_slide))
        .with_state(database);

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

async fn list_presentations(State(db): State<Database>) -> Json<Vec<Presentation>> {
    let db = db.read().await;
    Json(db.values().cloned().collect())
}

async fn create_presentation(
    State(db): State<Database>,
    Json(input): Json<CreatePresentation>,
) -> (StatusCode, Json<Presentation>) {
    let presentation = Presentation {
        id: Uuid::new_v4(),
        title: input.title,
        slides: Vec::new(),
    };

    db.write()
        .await
        .insert(presentation.id, presentation.clone());

    (StatusCode::CREATED, Json(presentation))
}

async fn get_presentation(
    Path(id): Path<Uuid>,
    State(db): State<Database>,
) -> Result<Json<Presentation>, StatusCode> {
    let db = db.read().await;

    db.get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn delete_presentation(
    Path(id): Path<Uuid>,
    State(db): State<Database>,
) -> StatusCode {
    let removed = db.write().await.remove(&id);

    if removed.is_some() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn add_slide(
    Path(id): Path<Uuid>,
    State(db): State<Database>,
    Json(input): Json<AddSlide>,
) -> Result<(StatusCode, Json<Slide>), StatusCode> {
    let mut db = db.write().await;

    let presentation = db.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;

    let slide = Slide {
        id: Uuid::new_v4(),
        title: input.title,
        content: input.content,
    };

    presentation.slides.push(slide.clone());

    Ok((StatusCode::CREATED, Json(slide)))
}
