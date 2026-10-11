mod data;

use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::Graphics::Gdi::{GetEnhMetaFileBits, HENHMETAFILE};
use windows::Win32::System::DataExchange::{
    EnumClipboardFormats, GetClipboardData, GetClipboardFormatNameW,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

use crate::data::{ClipboardItem, OpenedClipboard};

const CF_BITMAP: u32 = 2;
const CF_METAFILEPICT: u32 = 3;
const CF_UNICODETEXT: u32 = 13;
const CF_ENHMETAFILE: u32 = 14;
const CF_PALETTE: u32 = 9;
const CF_OWNERDISPLAY: u32 = 0x0080;
const CF_DSPBITMAP: u32 = 0x0082;
const CF_DSPMETAFILEPICT: u32 = 0x0083;
const CF_DSPENHMETAFILE: u32 = 0x008E;
const CF_GDIOBJ_RANGE: std::ops::RangeInclusive<u32> = 0x0300..=0x03FF;

#[derive(Debug, thiserror::Error)]
pub enum CbError<'a> {
    #[error("GlobalLock failed")]
    GlobalLockFailed,
    #[error("GetEnhMetaFileBits failed")]
    GetEnhMetaFileBitsFailed,
    #[error("Not supported type: {0}")]
    NotSupportedType(std::borrow::Cow<'a, str>),
    #[error("{0}")]
    WindowsError(#[from] windows::core::Error),
}

impl<'a> CbError<'a> {
    pub fn not_supported_type<'b: 'a, S>(t: S) -> Self
    where
        S: Into<std::borrow::Cow<'b, str>>,
    {
        Self::NotSupportedType(t.into())
    }
}

fn format_name(fmt: u32) -> String {
    let mut buf = [0_u16; 256];
    let len = unsafe { GetClipboardFormatNameW(fmt, &mut buf) };
    if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
    } else {
        fmt.to_string()
    }
}

unsafe fn read_hglobal<'a>(handle: HANDLE) -> Result<Vec<u8>, CbError<'a>> {
    let hglobal = HGLOBAL(handle.0);
    let size = unsafe { GlobalSize(hglobal) };
    let ptr = unsafe { GlobalLock(hglobal) };
    if ptr.is_null() {
        return Err(CbError::GlobalLockFailed);
    }
    let data = unsafe { std::slice::from_raw_parts(ptr as *const u8, size) }.to_vec();
    let _ = unsafe { GlobalUnlock(hglobal) };
    Ok(data)
}

unsafe fn read_enh_metafile<'a>(handle: HANDLE) -> Result<Vec<u8>, CbError<'a>> {
    let hemf = HENHMETAFILE(handle.0);
    let size = unsafe { GetEnhMetaFileBits(hemf, None) };
    if size == 0 {
        return Err(CbError::GetEnhMetaFileBitsFailed);
    }
    let mut data = vec![0_u8; size as usize];
    let written = unsafe { GetEnhMetaFileBits(hemf, Some(&mut data)) };
    if written == 0 {
        return Err(CbError::GetEnhMetaFileBitsFailed);
    }
    data.truncate(written as usize);
    Ok(data)
}

fn decode_utf16_text(data: &[u8]) -> String {
    let units: Vec<u16> = data
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

fn read_format<'a>(format: u32) -> Result<Vec<u8>, CbError<'a>> {
    match format {
        CF_BITMAP | CF_PALETTE | CF_DSPBITMAP => Err(CbError::not_supported_type("Not byte data")),
        CF_METAFILEPICT | CF_DSPMETAFILEPICT => {
            Err(CbError::not_supported_type("metafile picture"))
        }
        CF_OWNERDISPLAY => Err(CbError::not_supported_type("owner display")),
        f if CF_GDIOBJ_RANGE.contains(&f) => Err(CbError::not_supported_type(
            "GDI object range NOT contained",
        )),
        _ => unsafe {
            let handle = GetClipboardData(format)?;

            match format {
                CF_ENHMETAFILE | CF_DSPENHMETAFILE => read_enh_metafile(handle),
                _ => read_hglobal(handle),
            }
        },
    }
}

pub fn get_all_clipboard_items<'a>() -> windows::core::Result<Vec<ClipboardItem<'a>>> {
    let _lock = OpenedClipboard::open()?;

    let mut items = Vec::new();

    let mut format = 0u32;
    loop {
        format = unsafe { EnumClipboardFormats(format) };
        if format == 0 {
            break;
        }

        let name = format_name(format);
        items.push(match read_format(format) {
            Ok(data) if format == CF_UNICODETEXT => ClipboardItem::Text {
                format,
                name,
                text: decode_utf16_text(&data),
            },
            Ok(data) => ClipboardItem::Bytes { format, name, data },
            Err(reason) => ClipboardItem::Unavailable {
                format,
                name,
                reason,
            },
        });
    }

    Ok(items)
}
