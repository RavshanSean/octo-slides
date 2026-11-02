use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{net::SocketAddr, str::FromStr};
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

#[derive(Deserialize)]
struct UpdateSlide {
    title: String,
    content: String,
}

#[derive(Clone)]
struct AppState {
    sqlite: SqlitePool,
}

type ApiError = StatusCode;

fn database_error(error: sqlx::Error) -> ApiError {
    eprintln!("SQLite error: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

#[tokio::main]
async fn main() {
    let options = SqliteConnectOptions::from_str("sqlite://octo_slides.db")
        .expect("Invalid SQLite connection string")
        .create_if_missing(true)
        .foreign_keys(true);

    let sqlite = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("Failed to connect to SQLite");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS presentations (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL
        )
        "#,
    )
    .execute(&sqlite)
    .await
    .expect("Failed to create presentations table");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS slides (
            id TEXT PRIMARY KEY NOT NULL,
            presentation_id TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            FOREIGN KEY (presentation_id)
                REFERENCES presentations(id)
                ON DELETE CASCADE
        )
        "#,
    )
    .execute(&sqlite)
    .await
    .expect("Failed to create slides table");

    println!("SQLite database connected successfully!");

    let state = AppState { sqlite };

    let app = Router::new()
        .route("/health", get(health_check))
        .route(
            "/api/presentations",
            get(list_presentations).post(create_presentation),
        )
        .route(
            "/api/presentations/{id}",
            get(get_presentation).delete(delete_presentation),
        )
        .route(
            "/api/presentations/{id}/slides",
            axum::routing::post(add_slide),
        )
        .route(
            "/api/presentations/{id}/slides/{slide_id}",
            axum::routing::put(update_slide).delete(delete_slide),
        )
        .with_state(state);

    let address = SocketAddr::from(([127, 0, 0, 1], 3001));

    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("Failed to bind server");

    println!("Octo Slides API running at http://{address}");

    axum::serve(listener, app).await.expect("Server failed");
}

async fn health_check() -> &'static str {
    "Octo Slides API is healthy"
}

async fn load_presentation(
    sqlite: &SqlitePool,
    id: Uuid,
) -> Result<Option<Presentation>, ApiError> {
    let row = sqlx::query("SELECT id, title FROM presentations WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(sqlite)
        .await
        .map_err(database_error)?;

    let Some(row) = row else {
        return Ok(None);
    };

    let title: String = row.try_get("title").map_err(database_error)?;

    let slide_rows = sqlx::query(
        r#"
        SELECT id, title, content
        FROM slides
        WHERE presentation_id = ?
        ORDER BY rowid
        "#,
    )
    .bind(id.to_string())
    .fetch_all(sqlite)
    .await
    .map_err(database_error)?;

    let mut slides = Vec::new();

    for row in slide_rows {
        let slide_id: String = row.try_get("id").map_err(database_error)?;

        let slide_id = Uuid::parse_str(&slide_id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        slides.push(Slide {
            id: slide_id,
            title: row.try_get("title").map_err(database_error)?,
            content: row.try_get("content").map_err(database_error)?,
        });
    }

    Ok(Some(Presentation { id, title, slides }))
}

async fn list_presentations(
    State(state): State<AppState>,
) -> Result<Json<Vec<Presentation>>, ApiError> {
    let rows = sqlx::query("SELECT id FROM presentations ORDER BY rowid DESC")
        .fetch_all(&state.sqlite)
        .await
        .map_err(database_error)?;

    let mut presentations = Vec::new();

    for row in rows {
        let id: String = row.try_get("id").map_err(database_error)?;

        let id = Uuid::parse_str(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if let Some(presentation) = load_presentation(&state.sqlite, id).await? {
            presentations.push(presentation);
        }
    }

    Ok(Json(presentations))
}

async fn create_presentation(
    State(state): State<AppState>,
    Json(input): Json<CreatePresentation>,
) -> Result<(StatusCode, Json<Presentation>), ApiError> {
    if input.title.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let presentation = Presentation {
        id: Uuid::new_v4(),
        title: input.title,
        slides: Vec::new(),
    };

    sqlx::query("INSERT INTO presentations (id, title) VALUES (?, ?)")
        .bind(presentation.id.to_string())
        .bind(&presentation.title)
        .execute(&state.sqlite)
        .await
        .map_err(database_error)?;

    Ok((StatusCode::CREATED, Json(presentation)))
}

async fn get_presentation(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Presentation>, ApiError> {
    let presentation = load_presentation(&state.sqlite, id)
        .await?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(presentation))
}

async fn delete_presentation(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    let result = sqlx::query("DELETE FROM presentations WHERE id = ?")
        .bind(id.to_string())
        .execute(&state.sqlite)
        .await
        .map_err(database_error)?;

    if result.rows_affected() == 0 {
        Ok(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

async fn add_slide(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    Json(input): Json<AddSlide>,
) -> Result<(StatusCode, Json<Slide>), ApiError> {
    if input.title.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut tx = state.sqlite.begin().await.map_err(database_error)?;

    let exists = sqlx::query("SELECT id FROM presentations WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(&mut *tx)
        .await
        .map_err(database_error)?;

    if exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let slide = Slide {
        id: Uuid::new_v4(),
        title: input.title,
        content: input.content,
    };

    sqlx::query(
        r#"
        INSERT INTO slides (id, presentation_id, title, content)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(slide.id.to_string())
    .bind(id.to_string())
    .bind(&slide.title)
    .bind(&slide.content)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;

    tx.commit().await.map_err(database_error)?;

    Ok((StatusCode::CREATED, Json(slide)))
}

async fn update_slide(
    Path((id, slide_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    Json(input): Json<UpdateSlide>,
) -> Result<Json<Slide>, ApiError> {
    if input.title.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let result = sqlx::query(
        r#"
        UPDATE slides
        SET title = ?, content = ?
        WHERE id = ? AND presentation_id = ?
        "#,
    )
    .bind(&input.title)
    .bind(&input.content)
    .bind(slide_id.to_string())
    .bind(id.to_string())
    .execute(&state.sqlite)
    .await
    .map_err(database_error)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(Slide {
        id: slide_id,
        title: input.title,
        content: input.content,
    }))
}

async fn delete_slide(
    Path((id, slide_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    let result = sqlx::query(
        r#"
        DELETE FROM slides
        WHERE id = ? AND presentation_id = ?
        "#,
    )
    .bind(slide_id.to_string())
    .bind(id.to_string())
    .execute(&state.sqlite)
    .await
    .map_err(database_error)?;

    if result.rows_affected() == 0 {
        Ok(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_presentation_data() {
        let presentation = Presentation {
            id: Uuid::new_v4(),
            title: "Cybersecurity Fundamentals".to_string(),
            slides: Vec::new(),
        };

        assert_eq!(presentation.title, "Cybersecurity Fundamentals");
        assert!(presentation.slides.is_empty());
    }

    #[test]
    fn test_add_slide_data() {
        let mut presentation = Presentation {
            id: Uuid::new_v4(),
            title: "Cybersecurity".to_string(),
            slides: Vec::new(),
        };

        presentation.slides.push(Slide {
            id: Uuid::new_v4(),
            title: "Introduction".to_string(),
            content: "Understanding cybersecurity".to_string(),
        });

        assert_eq!(presentation.slides.len(), 1);
        assert_eq!(presentation.slides[0].title, "Introduction");
    }

    #[test]
    fn test_update_slide_data() {
        let mut slide = Slide {
            id: Uuid::new_v4(),
            title: "Old Title".to_string(),
            content: "Old Content".to_string(),
        };

        slide.title = "Updated Title".to_string();
        slide.content = "Updated Content".to_string();

        assert_eq!(slide.title, "Updated Title");
        assert_eq!(slide.content, "Updated Content");
    }

    #[test]
    fn test_delete_slide_data() {
        let slide_id = Uuid::new_v4();

        let mut slides = vec![Slide {
            id: slide_id,
            title: "Temporary Slide".to_string(),
            content: "Delete me".to_string(),
        }];

        slides.retain(|slide| slide.id != slide_id);

        assert!(slides.is_empty());
    }

    #[test]
    fn test_presentation_json_serialization() {
        let presentation = Presentation {
            id: Uuid::new_v4(),
            title: "AI Presentation".to_string(),
            slides: Vec::new(),
        };

        let json = serde_json::to_string(&presentation).expect("Serialization should succeed");

        assert!(json.contains("AI Presentation"));
        assert!(json.contains("\"slides\":[]"));
    }
}
