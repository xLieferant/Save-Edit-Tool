use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCode {
    ProfileNotFound,
    SaveNotFound,
    DecodeFailed,
    CompanyNotFoundInSave,
    CompanyHasNoJobOffers,
    CompanyHasNoJobOffersInCity,
    PostWriteValidationFailed,
    InvalidToken,
    WriteFailed,
    BackupFailed,
    LockTimeout,
    TelemetryUnavailable,
    JobLinkConflict,
    RollbackFailed,
    SteamCloudEnabled,
}

impl AppErrorCode {
    pub fn as_key(&self) -> &'static str {
        match self {
            Self::ProfileNotFound => "profile_not_found",
            Self::SaveNotFound => "save_not_found",
            Self::DecodeFailed => "decode_failed",
            Self::CompanyNotFoundInSave => "company_not_found_in_save",
            Self::CompanyHasNoJobOffers => "company_has_no_job_offers",
            Self::CompanyHasNoJobOffersInCity => "company_has_no_job_offers_in_city",
            Self::PostWriteValidationFailed => "post_write_validation_failed",
            Self::InvalidToken => "invalid_token",
            Self::WriteFailed => "write_failed",
            Self::BackupFailed => "backup_failed",
            Self::LockTimeout => "lock_timeout",
            Self::TelemetryUnavailable => "telemetry_unavailable",
            Self::JobLinkConflict => "job_link_conflict",
            Self::RollbackFailed => "rollback_failed",
            Self::SteamCloudEnabled => "steam_cloud_enabled",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
}

impl AppError {
    pub fn new(code: AppErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::new(AppErrorCode::WriteFailed, error.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::new(AppErrorCode::WriteFailed, error.to_string())
    }
}
