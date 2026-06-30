use axum::{extract::State, Extension, Json};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::attest_guard::VerifiedDevice;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AiProxyRequest {
    pub prompt: String,
    pub execution_token: String,
}

/// Handler for `POST /api/v1/ai/proxy`.
/// Strips all client-identifying details (IP, headers, UUIDs) and forwards the prompt to Claude API.
/// If `ANTHROPIC_API_KEY` is not configured and the environment is `development`, returns a mock response.
#[tracing::instrument(skip(state, payload), fields(execution_token = %payload.execution_token))]
pub async fn ai_proxy_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
    Json(payload): Json<AiProxyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/ai/proxy").increment(1);
    let start_time = std::time::Instant::now();

    // 1. Audit log the access of the AI proxy
    state
        .db
        .insert_audit_log("AI_PROXY", verified_device.device_id, Uuid::nil(), &[])
        .await?;

    // 2. If key is missing, handle gracefully in development mode, otherwise fail.
    if state.config.ai.anthropic_api_key.is_empty() {
        if state.config.auth.environment == "development" {
            tracing::info!(
                "Anthropic API key is empty; returning mock response in development environment"
            );
            metrics::histogram!("antigravity_request_duration_seconds", "endpoint" => "/ai/proxy")
                .record(start_time.elapsed().as_secs_f64());
            return Ok(Json(json!({
                "id": "msg_mock_dev_12345",
                "type": "message",
                "role": "assistant",
                "content": [
                    {
                        "type": "text",
                        "text": format!("[Mock Claude Response] Received prompt: \"{}\" with execution token: {}", payload.prompt, payload.execution_token)
                    }
                ],
                "model": "claude-3-5-sonnet-20241022",
                "usage": {
                    "input_tokens": 12,
                    "output_tokens": 35
                }
            })));
        } else {
            metrics::counter!("antigravity_api_errors_total", "endpoint" => "/ai/proxy", "error" => "missing_api_key").increment(1);
            return Err(AppError::ExternalServiceError(
                "Anthropic API key is not configured".to_owned(),
            ));
        }
    }

    // outbound messages request structure for Claude API
    let outbound_body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "max_tokens": 2048,
        "messages": [
            {
                "role": "user",
                "content": payload.prompt
            }
        ],
        "system": "You are Lifeline AI, a private health companion. Under no circumstances should you ask for, collect, or log identifying user information. Provide clinical-first advice based on the biometric metrics provided."
    });

    let ai_start = std::time::Instant::now();
    // 3. Make anonymized outbound call to Anthropic API (IP/metadata of the client is stripped)
    let response = state
        .http_client
        .post(&state.config.ai.anthropic_api_url)
        .header("x-api-key", &state.config.ai.anthropic_api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&outbound_body)
        .send()
        .await
        .map_err(|e| {
            AppError::ExternalServiceError(format!("Failed to connect to Anthropic: {e}"))
        })?;

    metrics::histogram!("antigravity_ai_latency_seconds", "model" => "claude-3-5-sonnet")
        .record(ai_start.elapsed().as_secs_f64());

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        metrics::counter!("antigravity_api_errors_total", "endpoint" => "/ai/proxy", "error" => "anthropic_failure").increment(1);
        return Err(AppError::ExternalServiceError(format!(
            "Anthropic API returned status {status}: {error_body}"
        )));
    }

    let response_json: serde_json::Value = response.json().await.map_err(|e| {
        AppError::ExternalServiceError(format!("Failed to parse Anthropic JSON response: {e}"))
    })?;

    metrics::histogram!("antigravity_request_duration_seconds", "endpoint" => "/ai/proxy")
        .record(start_time.elapsed().as_secs_f64());

    Ok(Json(response_json))
}

/// Handler for `GET /api/v1/ai/policy-matrix`.
/// Serves the latest health assistance behavior matrix and system prompt templates.
#[tracing::instrument(skip(state))]
pub async fn policy_matrix_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/ai/policy-matrix")
        .increment(1);
    Ok(Json(json!({
        "version": state.config.ai.policy_matrix_version,
        "system_prompt": "You are Lifeline AI, a private health companion. You analyze biometric data locally and act on the user's behalf. You do not store or track any user identifiers.",
        "behavior_model": "clinical-first-empathy",
        "habit_optimization": {
            "sleep": {
                "ideal_winddown_minutes": 60,
                "screen_time_limit_minutes": 30,
                "caffeine_cutoff_hours": 10
            },
            "activity": {
                "hourly_stand_interval_minutes": 50,
                "cardio_zones": [3, 4]
            }
        }
    })))
}
