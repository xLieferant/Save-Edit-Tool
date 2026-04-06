use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatusPayload {
    pub sdk_active: bool,
    pub telemetry_available: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryJobEventPayload {
    pub sdk_active: bool,
    pub paused: bool,
    pub on_job: bool,
    pub job_finished: bool,
    pub job_delivered: bool,
    pub cargo_id: Option<String>,
    pub cargo: Option<String>,
    pub city_src_id: Option<String>,
    pub city_src: Option<String>,
    pub comp_src_id: Option<String>,
    pub comp_src: Option<String>,
    pub city_dst_id: Option<String>,
    pub city_dst: Option<String>,
    pub comp_dst_id: Option<String>,
    pub comp_dst: Option<String>,
    pub planned_distance_km: f64,
    pub route_distance: f64,
    pub route_time: i64,
    pub job_income: i64,
    pub job_delivered_revenue: i64,
}
