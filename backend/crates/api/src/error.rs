use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use shared::AppError as DomainError;

pub struct ApiError(pub DomainError);

impl From<DomainError> for ApiError {
    fn from(e: DomainError) -> Self {
        ApiError(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self.0 {
            DomainError::Database(_) | DomainError::Migration(_) | DomainError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string())
            }
            DomainError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            DomainError::NotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
        };
        tracing::error!(error = %self.0, status = %status, "request failed");
        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub type AppResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_error_maps_to_500() {
        let err: ApiError = DomainError::Database(sqlx::Error::PoolClosed).into();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn bad_request_maps_to_400() {
        let err: ApiError = DomainError::BadRequest("missing field".to_string()).into();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn not_found_maps_to_404() {
        let err: ApiError = DomainError::NotFound("widget".to_string()).into();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
