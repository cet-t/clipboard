//! These tests write to the real system clipboard and read it back through
//! the Win+V history, so they require clipboard history to be enabled and
//! they overwrite the current clipboard. They are serialized with a lock.

use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use windows::ApplicationModel::DataTransfer::{Clipboard, DataPackage, HtmlFormatHelper};
use windows::Storage::Streams::{
    DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference,
};
use windows::core::HSTRING;

use crate::sta::run_sta;
use crate::{ContentData, HistoryEntry, get_clipboard_history, is_history_enabled};

static CLIPBOARD_LOCK: Mutex<()> = Mutex::new(());

fn unique(tag: &str) -> String {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("clipboard-test-{tag}-{n}")
}

/// Puts a package built by `build` on the clipboard.
fn set_clipboard(build: impl FnOnce(&DataPackage) -> windows::core::Result<()> + Send) {
    run_sta(|| {
        let package = DataPackage::new()?;
        build(&package)?;
        Clipboard::SetContent(&package)?;
        Clipboard::Flush()
    })
    .expect("failed to set clipboard content");
}

/// Polls the history until the newest entry satisfies `pred`.
fn wait_newest(pred: impl Fn(&HistoryEntry) -> bool) -> HistoryEntry {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let entries = get_clipboard_history().unwrap();
        if let Some(e) = entries.into_iter().next().filter(|e| pred(e)) {
            return e;
        }
        assert!(Instant::now() < deadline, "timed out waiting for history");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn find<'a>(e: &'a HistoryEntry, format: &str) -> &'a ContentData {
    &e.contents
        .iter()
        .find(|c| c.format == format)
        .unwrap_or_else(|| panic!("format {format} not found in {:?}", e.contents))
        .data
}

fn has_text(e: &HistoryEntry, s: &str) -> bool {
    e.contents
        .iter()
        .any(|c| matches!(&c.data, ContentData::Text(t) if t == s))
}

fn guard() -> std::sync::MutexGuard<'static, ()> {
    let g = CLIPBOARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    assert!(
        is_history_enabled().unwrap(),
        "Win+V の履歴を有効にしてください"
    );
    g
}

/// 1x1 24-bit BMP.
fn tiny_bmp() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&58u32.to_le_bytes()); // file size
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&54u32.to_le_bytes()); // pixel data offset
    b.extend_from_slice(&40u32.to_le_bytes()); // DIB header size
    b.extend_from_slice(&1i32.to_le_bytes()); // width
    b.extend_from_slice(&1i32.to_le_bytes()); // height
    b.extend_from_slice(&1u16.to_le_bytes()); // planes
    b.extend_from_slice(&24u16.to_le_bytes()); // bpp
    b.extend_from_slice(&[0; 24]); // compression..colors (all zero)
    b.extend_from_slice(&[0x00, 0x00, 0xFF, 0x00]); // one red pixel + padding
    b
}

#[test]
fn history_is_readable() {
    let _g = guard();
    let entries = get_clipboard_history().unwrap();
    assert!(!entries.is_empty());
    for e in &entries {
        assert!(!e.id.is_empty());
        assert!(!e.contents.is_empty(), "entry {} has no formats", e.id);
    }
}

#[test]
fn text_round_trip() {
    let _g = guard();
    let text = unique("text");
    set_clipboard(|p| p.SetText(&HSTRING::from(&text)));

    let e = wait_newest(|e| has_text(e, &text));
    assert!(matches!(find(&e, "Text"), ContentData::Text(t) if *t == text));
}

#[test]
fn unicode_text_round_trip() {
    let _g = guard();
    let text = format!("{} 日本語 😀 \r\n改行", unique("unicode"));
    set_clipboard(|p| p.SetText(&HSTRING::from(&text)));

    wait_newest(|e| has_text(e, &text));
}

#[test]
fn html_round_trip() {
    let _g = guard();
    let marker = unique("html");
    let html =
        HtmlFormatHelper::CreateHtmlFormat(&HSTRING::from(format!("<b>{marker}</b>"))).unwrap();
    set_clipboard(|p| p.SetHtmlFormat(&html));

    let e = wait_newest(|e| {
        e.contents
            .iter()
            .any(|c| matches!(&c.data, ContentData::Html(h) if h.contains(&marker)))
    });
    assert!(matches!(find(&e, "HTML Format"), ContentData::Html(h) if h.contains("<b>")));
}

#[test]
fn bitmap_round_trip() {
    let _g = guard();
    set_clipboard(|p| {
        let stream = InMemoryRandomAccessStream::new()?;
        let writer = DataWriter::CreateDataWriter(&stream)?;
        writer.WriteBytes(&tiny_bmp())?;
        writer.StoreAsync()?.join()?;
        writer.FlushAsync()?.join()?;
        writer.DetachStream()?;
        stream.Seek(0)?;
        p.SetBitmap(&RandomAccessStreamReference::CreateFromStream(&stream)?)
    });

    let e = wait_newest(|e| {
        e.contents
            .iter()
            .any(|c| matches!(c.data, ContentData::Bitmap { .. }))
    });
    match find(&e, "Bitmap") {
        ContentData::Bitmap { mime, bytes } => {
            assert!(mime.starts_with("image/"), "unexpected mime {mime}");
            assert!(!bytes.is_empty());
        }
        other => panic!("expected bitmap, got {other:?}"),
    }
}

#[test]
fn newest_entry_comes_first() {
    let _g = guard();
    let first = unique("first");
    let second = unique("second");

    set_clipboard(|p| p.SetText(&HSTRING::from(&first)));
    wait_newest(|e| has_text(e, &first));
    set_clipboard(|p| p.SetText(&HSTRING::from(&second)));
    wait_newest(|e| has_text(e, &second));

    let entries = get_clipboard_history().unwrap();
    let pos = |s| entries.iter().position(|e| has_text(e, s));
    let (p2, p1) = (pos(&second).unwrap(), pos(&first).unwrap());
    assert!(p2 < p1, "newer entry must come before older one");
    assert!(entries[p2].timestamp_ms >= entries[p1].timestamp_ms);
}

#[test]
fn entry_metadata_is_sane() {
    let _g = guard();
    let text = unique("meta");
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    set_clipboard(|p| p.SetText(&HSTRING::from(&text)));

    let e = wait_newest(|e| has_text(e, &text));
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    assert!(!e.id.is_empty());
    assert!(
        (before - 2_000..=after + 2_000).contains(&e.timestamp_ms),
        "timestamp {} not in [{before}, {after}]",
        e.timestamp_ms
    );
}
