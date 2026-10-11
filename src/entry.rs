#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub id: String,
    pub timestamp_ms: i64,
    pub content: std::sync::Arc<ContentData>,
}

#[derive(Debug, Clone)]
pub enum ContentData {
    /// Text, HTML, RTF, URI and file paths (one per line).
    Text(String),
    Image {
        mime: String,
        bytes: Vec<u8>,
    },
    /// Any other format, or one that could not be read (empty).
    Other(Vec<u8>),
}
