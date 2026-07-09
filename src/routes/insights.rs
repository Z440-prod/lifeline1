use axum::{extract::State, Json};
use serde_json::json;
use std::sync::Arc;

use crate::errors::AppError;
use crate::state::AppState;

/// Handler for `GET /api/v1/insights/config`.
///
/// Ships the **rules** for Lifeline's on-device insights engine — never any
/// user data. This is what keeps the differentiated longevity features
/// (Lifeline Age, cross-provider Readiness, biomarker reference ranges,
/// behavioral-correlation weighting, circadian windows) compatible with the
/// zero-knowledge architecture: the server publishes the model coefficients
/// and reference ranges, and the client applies them to plaintext health data
/// that the server never sees. Public + cacheable, mirroring
/// `GET /ai/policy-matrix`.
#[tracing::instrument(skip(state))]
pub async fn insights_config_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/insights/config")
        .increment(1);

    Ok(Json(json!({
        "version": "1.0.0",
        "policy_matrix_version": state.config.ai.policy_matrix_version,

        // ── Lifeline Age ─────────────────────────────────────────────────────
        // A transparent, additive biological-age model. The client starts from
        // chronological age and applies year offsets per signal band. Kept
        // intentionally simple and inspectable rather than a black box.
        "biological_age": {
            "baseline": "chronological",
            "signals": {
                "resting_heart_rate": {
                    "unit": "bpm",
                    "bands": [
                        { "max": 55, "years": -3.0 },
                        { "max": 65, "years": -1.0 },
                        { "max": 75, "years": 1.0 },
                        { "max": 200, "years": 3.5 }
                    ]
                },
                "hrv_ms": {
                    "unit": "ms",
                    "bands": [
                        { "max": 30, "years": 3.0 },
                        { "max": 50, "years": 1.0 },
                        { "max": 80, "years": -1.5 },
                        { "max": 500, "years": -3.0 }
                    ]
                },
                "sleep_hours": {
                    "unit": "h",
                    "bands": [
                        { "max": 6.0, "years": 2.0 },
                        { "max": 7.0, "years": 0.5 },
                        { "max": 9.0, "years": -1.5 },
                        { "max": 24.0, "years": 1.0 }
                    ]
                },
                "daily_steps": {
                    "unit": "steps",
                    "bands": [
                        { "max": 4000, "years": 2.0 },
                        { "max": 8000, "years": 0.0 },
                        { "max": 12000, "years": -1.5 },
                        { "max": 100000, "years": -2.5 }
                    ]
                }
            },
            "clamp_years": 12.0
        },

        // ── Cross-provider Readiness ────────────────────────────────────────
        // A single 0–100 readiness fused from whatever the user has connected.
        // Weights are renormalized on-device over available components so a
        // user with only Apple Health still gets a coherent score.
        "readiness": {
            "components": {
                "hrv": { "weight": 0.35, "good_at": 70, "poor_at": 30 },
                "resting_heart_rate": { "weight": 0.20, "good_at": 52, "poor_at": 80, "invert": true },
                "sleep_performance": { "weight": 0.30, "good_at": 95, "poor_at": 60 },
                "prior_strain": { "weight": 0.15, "good_at": 8, "poor_at": 18, "invert": true }
            },
            "labels": [
                { "min": 80, "text": "Primed" },
                { "min": 60, "text": "Ready" },
                { "min": 40, "text": "Maintain" },
                { "min": 0, "text": "Recover" }
            ]
        },

        // ── Biomarker reference ranges ──────────────────────────────────────
        // For labs the user uploads. The server ships the ranges; the client
        // decrypts the lab values on-device and flags out-of-range markers.
        "biomarkers": {
            "ldl_cholesterol": { "unit": "mg/dL", "optimal_max": 100, "borderline_max": 130, "high_min": 160 },
            "hdl_cholesterol": { "unit": "mg/dL", "low_max": 40, "optimal_min": 60 },
            "hba1c": { "unit": "%", "optimal_max": 5.4, "prediabetic_min": 5.7, "diabetic_min": 6.5 },
            "vitamin_d": { "unit": "ng/mL", "deficient_max": 20, "optimal_min": 40 },
            "tsh": { "unit": "mIU/L", "optimal_min": 0.4, "optimal_max": 4.0 },
            "fasting_glucose": { "unit": "mg/dL", "optimal_max": 99, "prediabetic_min": 100, "diabetic_min": 126 }
        },

        // ── Behavioral-feedback correlation weighting ───────────────────────
        // Prior weights for "what's moving your score", refined on-device by
        // the user's own habit-vs-outcome history.
        "correlation": {
            "habits": {
                "morning_walk": { "prior": 0.62, "targets": ["daily_steps", "resting_heart_rate"] },
                "medication_adherence": { "prior": 0.55, "targets": ["biomarkers"] },
                "winddown_routine": { "prior": 0.71, "targets": ["sleep_hours", "hrv_ms"] },
                "hydration": { "prior": 0.34, "targets": ["hrv_ms"] }
            },
            "min_observations": 5
        },

        // ── Circadian windows ───────────────────────────────────────────────
        // Chronotype-aware default timing windows the client shifts by the
        // user's measured sleep midpoint.
        "circadian": {
            "chronotypes": {
                "lark": { "wind_down": "20:30", "peak_focus": "09:00", "last_caffeine": "12:00" },
                "neutral": { "wind_down": "22:00", "peak_focus": "10:30", "last_caffeine": "14:00" },
                "owl": { "wind_down": "23:30", "peak_focus": "12:30", "last_caffeine": "16:00" }
            }
        }
    })))
}
