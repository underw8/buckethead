use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("AWS error: {message} (code: {code})")]
    Aws { code: String, message: String },
    #[error("Credentials error: {0}")]
    Credentials(String),
    #[error("SSO token expired — run: aws sso login --profile {profile}")]
    SsoExpired { profile: String },
    #[error("Access denied: {0}")]
    AccessDenied(String),
    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = s.serialize_map(Some(2))?;
        let code = match self {
            AppError::SsoExpired { .. } => "SSO_EXPIRED",
            AppError::Credentials(_) => "CREDENTIALS_ERROR",
            AppError::AccessDenied(_) => "ACCESS_DENIED",
            AppError::Aws { code, .. } => code.as_str(),
            AppError::Other(_) => "ERROR",
        };
        map.serialize_entry("code", code)?;
        map.serialize_entry("message", &self.to_string())?;
        map.end()
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        let lower = s.to_lowercase();
        if (lower.contains("token") || lower.contains("sso")) && lower.contains("expir") {
            AppError::Credentials(s)
        } else if lower.contains("access denied") || lower.contains("accessdenied") {
            AppError::AccessDenied(s)
        } else {
            AppError::Other(s)
        }
    }
}
