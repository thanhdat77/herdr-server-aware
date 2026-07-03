use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PickerItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub path: String,
    pub kind: String,
}
