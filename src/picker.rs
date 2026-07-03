use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickerItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub path: String,
    pub kind: String,
}
