use windows::ApplicationModel::DataTransfer::{
    Clipboard, ClipboardHistoryItem, ClipboardHistoryItemsResultStatus,
};
use windows::core::{Error, Result};

use crate::content::read_all;
use crate::entry::HistoryEntry;
use crate::sta::{E_FAIL, run_sta};

const EPOCH_DIFF_100NS: i64 = 116_444_736_000_000_000;

pub fn is_history_enabled() -> Result<bool> {
    run_sta(Clipboard::IsHistoryEnabled)
}

pub fn get_clipboard_history() -> Result<Vec<HistoryEntry>> {
    run_sta(|| {
        let result = Clipboard::GetHistoryItemsAsync()?.join()?;
        match result.Status()? {
            ClipboardHistoryItemsResultStatus::Success => {
                result.Items()?.into_iter().map(read_entry).collect()
            }
            ClipboardHistoryItemsResultStatus::ClipboardHistoryDisabled => {
                Err(Error::new(E_FAIL, "clipboard history is disabled"))
            }
            ClipboardHistoryItemsResultStatus::AccessDenied => {
                Err(Error::new(E_FAIL, "clipboard history access denied"))
            }
            _ => Err(Error::new(E_FAIL, "failed to get clipboard history")),
        }
    })
}

fn read_entry(item: ClipboardHistoryItem) -> Result<HistoryEntry> {
    Ok(HistoryEntry {
        id: item.Id()?.to_string(),
        timestamp_ms: (item.Timestamp()?.UniversalTime - EPOCH_DIFF_100NS) / 10_000,
        contents: read_all(&item.Content()?)?,
    })
}
