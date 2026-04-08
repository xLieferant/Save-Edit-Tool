use serde::{Deserialize, Serialize};

use crate::models::cached_profile::CachedProfile;
use crate::models::clone_profiles_info::Profile;
use crate::models::profile_info::ProfileInfo;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileIdentity {
    pub id: Option<String>,
    pub path: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileLoadState {
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSaveSelection {
    pub profile_path: Option<String>,
    pub save_path: Option<String>,
}

impl ActiveSaveSelection {
    pub fn is_ready(&self) -> bool {
        self.profile_path.is_some() && self.save_path.is_some()
    }
}

impl From<Profile> for ProfileIdentity {
    fn from(value: Profile) -> Self {
        Self {
            id: Some(value.id),
            path: value.path.to_string_lossy().to_string(),
            name: Some(value.name),
        }
    }
}

impl From<ProfileInfo> for ProfileIdentity {
    fn from(value: ProfileInfo) -> Self {
        Self {
            id: None,
            path: value.path,
            name: value.name,
        }
    }
}

impl From<CachedProfile> for ProfileIdentity {
    fn from(value: CachedProfile) -> Self {
        Self {
            id: None,
            path: value.path,
            name: value.name,
        }
    }
}

impl From<ProfileInfo> for ProfileLoadState {
    fn from(value: ProfileInfo) -> Self {
        Self {
            success: value.success,
            message: value.message,
        }
    }
}

impl From<CachedProfile> for ProfileLoadState {
    fn from(value: CachedProfile) -> Self {
        Self {
            success: value.success,
            message: value.message,
        }
    }
}
