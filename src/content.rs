use windows::ApplicationModel::DataTransfer::{DataPackageView, StandardDataFormats};
use windows::Foundation::{IPropertyValue, PropertyType};
use windows::Storage::Streams::{DataReader, IRandomAccessStream, RandomAccessStreamReference};
use windows::core::{Array, Error, HSTRING, Interface, Result};

use crate::entry::ContentData;
use crate::sta::E_FAIL;

/// Reads one representative content: Text, then Image, then any other format.
pub(crate) fn read_content(view: &DataPackageView) -> Result<ContentData> {
    let text = StandardDataFormats::Text()?;
    let bitmap = StandardDataFormats::Bitmap()?;
    let mut formats: Vec<HSTRING> = view.AvailableFormats()?.into_iter().collect();
    formats.sort_by_key(|f| match f {
        f if *f == text => 0,
        f if *f == bitmap => 1,
        _ => 2,
    });
    for name in &formats {
        match read_standard(view, name) {
            Ok(Some(data)) => return Ok(data),
            Ok(None) => {
                if let Ok(data) = read_raw(view, name) {
                    return Ok(data);
                }
            }
            Err(_) => {}
        }
    }
    Ok(ContentData::Other(Vec::new()))
}

fn read_standard(view: &DataPackageView, name: &HSTRING) -> Result<Option<ContentData>> {
    let name = name.to_owned();
    if name == StandardDataFormats::Text()? {
        Ok(ContentData::Text(view.GetTextAsync()?.join()?.to_string()))
    } else if name == StandardDataFormats::Html()? {
        Ok(ContentData::Text(
            view.GetHtmlFormatAsync()?.join()?.to_string(),
        ))
    } else if name == StandardDataFormats::Rtf()? {
        Ok(ContentData::Text(view.GetRtfAsync()?.join()?.to_string()))
    } else if name == StandardDataFormats::Uri()? {
        Ok(ContentData::Text(
            view.GetUriAsync()?.join()?.AbsoluteUri()?.to_string(),
        ))
    } else if name == StandardDataFormats::Bitmap()? {
        let stream = view.GetBitmapAsync()?.join()?.OpenReadAsync()?.join()?;
        let mime = stream.ContentType()?.to_string();
        Ok(ContentData::Image {
            mime,
            bytes: read_stream(&stream)?,
        })
    } else if name == StandardDataFormats::StorageItems()? {
        let items = view
            .GetStorageItemsAsync()?
            .join()?
            .into_iter()
            .filter_map(|i| i.Path().ok())
            .map(|i| i.to_string())
            .collect::<Vec<_>>();
        Ok(ContentData::Text(items.join("\n")))
    } else {
        return Ok(None);
    }
    .map(Some)
}

fn read_stream(stream: &impl Interface) -> Result<Vec<u8>> {
    let stream = stream.cast::<IRandomAccessStream>()?;
    let size = stream.Size()? as u32;
    let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    reader.LoadAsync(size)?.join()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes)?;
    Ok(bytes)
}

fn read_raw(view: &DataPackageView, name: &HSTRING) -> Result<ContentData> {
    let value = view.GetDataAsync(name)?.join()?;
    if let Ok(stream) = value.cast::<IRandomAccessStream>() {
        return Ok(ContentData::Other(read_stream(&stream)?));
    }
    if let Ok(reference) = value.cast::<RandomAccessStreamReference>() {
        return Ok(ContentData::Other(read_stream(
            &reference.OpenReadAsync()?.join()?,
        )?));
    }
    let prop = value.cast::<IPropertyValue>()?;
    let bytes = match prop.Type()? {
        PropertyType::UInt8Array => {
            let mut arr = Array::<u8>::new();
            prop.GetUInt8Array(&mut arr)?;
            arr.to_vec()
        }
        PropertyType::String => return Ok(ContentData::Text(prop.GetString()?.to_string())),
        PropertyType::UInt8 => vec![prop.GetUInt8()?],
        PropertyType::Boolean => vec![prop.GetBoolean()? as u8],
        PropertyType::Int16 => prop.GetInt16()?.to_le_bytes().to_vec(),
        PropertyType::UInt16 | PropertyType::Char16 => prop.GetUInt16()?.to_le_bytes().to_vec(),
        PropertyType::Int32 => prop.GetInt32()?.to_le_bytes().to_vec(),
        PropertyType::UInt32 => prop.GetUInt32()?.to_le_bytes().to_vec(),
        PropertyType::Int64 => prop.GetInt64()?.to_le_bytes().to_vec(),
        PropertyType::UInt64 => prop.GetUInt64()?.to_le_bytes().to_vec(),
        PropertyType::Single => prop.GetSingle()?.to_le_bytes().to_vec(),
        PropertyType::Double => prop.GetDouble()?.to_le_bytes().to_vec(),
        t => {
            return Err(Error::new(
                E_FAIL,
                format!("unsupported property type {}", t.0),
            ));
        }
    };
    Ok(ContentData::Other(bytes))
}
