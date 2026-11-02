use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideDesign {
    pub slide_id: Uuid,
    pub width: u32,
    pub height: u32,
    pub background: String,
    pub elements: Vec<SlideElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideElement {
    pub id: Uuid,
    pub kind: ElementKind,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
    pub z_index: i32,
    pub locked: bool,
    pub opacity: f64,
    pub content: Value,
    pub style: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementKind {
    Text,
    Image,
    Shape,
    Icon,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDesign {
    pub width: u32,
    pub height: u32,
    pub background: String,
    pub elements: Vec<SlideElement>,
}

#[derive(Clone)]
pub struct DesignState {
    pub db: SqlitePool,
}

pub async fn initialize(db: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS slide_designs (
            slide_id TEXT PRIMARY KEY NOT NULL,
            design_json TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (slide_id)
                REFERENCES slides(id)
                ON DELETE CASCADE
        )",
    )
    .execute(db)
    .await?;

    Ok(())
}

pub fn routes(db: SqlitePool) -> Router {
    Router::new()
        .route(
            "/api/slides/{slide_id}/design",
            get(get_design).put(save_design),
        )
        .with_state(DesignState { db })
}

fn validate(input: &SaveDesign) -> Result<(), &'static str> {
    if input.width == 0 || input.height == 0 {
        return Err("Slide dimensions must be positive");
    }

    if input.width > 8192 || input.height > 8192 {
        return Err("Slide dimensions exceed supported limits");
    }

    if input.background.is_empty() || input.background.len() > 128 {
        return Err("Invalid slide background");
    }

    if input.elements.len() > 500 {
        return Err("Too many slide elements");
    }

    let mut ids = std::collections::HashSet::new();

    for element in &input.elements {
        if !ids.insert(element.id) {
            return Err("Duplicate element ID");
        }

        let values = [
            element.x,
            element.y,
            element.width,
            element.height,
            element.rotation,
            element.opacity,
        ];

        if values.iter().any(|value| !value.is_finite()) {
            return Err("Element contains invalid numeric values");
        }

        if element.width <= 0.0 || element.height <= 0.0 {
            return Err("Element dimensions must be positive");
        }

        if !(0.0..=1.0).contains(&element.opacity) {
            return Err("Element opacity must be between 0 and 1");
        }

        if !element.content.is_object() || !element.style.is_object() {
            return Err("Element content and style must be objects");
        }
    }

    Ok(())
}

async fn get_design(
    State(state): State<DesignState>,
    Path(slide_id): Path<Uuid>,
) -> Result<Json<SlideDesign>, StatusCode> {
    let row = sqlx::query("SELECT design_json FROM slide_designs WHERE slide_id = ?")
        .bind(slide_id.to_string())
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let row = row.ok_or(StatusCode::NOT_FOUND)?;

    let raw: String = row
        .try_get("design_json")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let design =
        serde_json::from_str::<SlideDesign>(&raw).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(design))
}

async fn save_design(
    State(state): State<DesignState>,
    Path(slide_id): Path<Uuid>,
    Json(input): Json<SaveDesign>,
) -> Result<Json<SlideDesign>, StatusCode> {
    validate(&input).map_err(|_| StatusCode::BAD_REQUEST)?;

    let exists = sqlx::query("SELECT id FROM slides WHERE id = ?")
        .bind(slide_id.to_string())
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let design = SlideDesign {
        slide_id,
        width: input.width,
        height: input.height,
        background: input.background,
        elements: input.elements,
    };

    let json = serde_json::to_string(&design).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    sqlx::query(
        "INSERT INTO slide_designs (slide_id, design_json)
         VALUES (?, ?)
         ON CONFLICT(slide_id)
         DO UPDATE SET
             design_json = excluded.design_json,
             updated_at = CURRENT_TIMESTAMP",
    )
    .bind(slide_id.to_string())
    .bind(json)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(design))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SaveDesign {
        SaveDesign {
            width: 1920,
            height: 1080,
            background: "#101827".into(),
            elements: vec![SlideElement {
                id: Uuid::new_v4(),
                kind: ElementKind::Text,
                x: 100.0,
                y: 100.0,
                width: 800.0,
                height: 120.0,
                rotation: 0.0,
                z_index: 1,
                locked: false,
                opacity: 1.0,
                content: serde_json::json!({
                    "text": "Introduction to Cybersecurity"
                }),
                style: serde_json::json!({
                    "fontFamily": "Inter",
                    "fontSize": 64,
                    "fontWeight": 700,
                    "color": "#FFFFFF"
                }),
            }],
        }
    }

    #[test]
    fn valid_design_passes() {
        assert!(validate(&sample()).is_ok());
    }

    #[test]
    fn zero_dimensions_rejected() {
        let mut design = sample();
        design.width = 0;
        assert!(validate(&design).is_err());
    }

    #[test]
    fn invalid_opacity_rejected() {
        let mut design = sample();
        design.elements[0].opacity = 1.5;
        assert!(validate(&design).is_err());
    }

    #[test]
    fn duplicate_ids_rejected() {
        let mut design = sample();
        let duplicate = design.elements[0].clone();
        design.elements.push(duplicate);
        assert!(validate(&design).is_err());
    }

    #[test]
    fn design_serializes_correctly() {
        let design = sample();
        let json = serde_json::to_value(&design).unwrap();

        assert_eq!(json["width"], 1920);
        assert_eq!(json["elements"][0]["kind"], "text");
    }
}
