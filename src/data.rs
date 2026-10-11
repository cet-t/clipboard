use std::{thread, time::Duration};

use windows::Win32::{
    Foundation::HWND,
    System::DataExchange::{CloseClipboard, OpenClipboard},
};

use crate::CbError;

#[derive(Debug)]
pub enum ClipboardItem<'a> {
    Text {
        format: u32,
        name: String,
        text: String,
    },
    Bytes {
        format: u32,
        name: String,
        data: Vec<u8>,
    },
    Unavailable {
        format: u32,
        name: String,
        reason: CbError<'a>,
    },
}

impl ClipboardItem<'_> {
    pub fn format(&self) -> u32 {
        match self {
            Self::Text { format, .. }
            | Self::Bytes { format, .. }
            | Self::Unavailable { format, .. } => format.to_owned(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Text { name, .. } | Self::Bytes { name, .. } | Self::Unavailable { name, .. } => {
                name
            }
        }
    }
}

/// 開いている間だけクリップボードを保持し、Drop で閉じる。
pub(crate) struct OpenedClipboard;

impl OpenedClipboard {
    pub(crate) fn open() -> windows::core::Result<Self> {
        let mut last = Ok(());
        for _ in 0..10 {
            match unsafe { OpenClipboard(Some(HWND::default())) } {
                Ok(()) => return Ok(Self),
                Err(e) => last = Err(e),
            }
            thread::sleep(Duration::from_millis(10));
        }
        last.map(|()| Self)
    }
}

impl Drop for OpenedClipboard {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}
