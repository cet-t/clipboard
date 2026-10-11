use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub id: String,
    pub timestamp_ms: i64,
    pub contents: Vec<Rc<Content>>,
}

#[derive(Debug, Clone)]
pub struct Content {
    pub format: String,
    pub data: ContentData,
}

#[derive(Debug, Clone)]
pub enum ContentData {
    Text(String),
    Html(String),
    Rtf(String),
    Uri(String),
    Bitmap { mime: String, bytes: Vec<u8> },
    Files(Vec<String>),
    Bytes(Vec<u8>),
    Unavailable,
}
