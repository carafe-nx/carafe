//! Windows Portable Devices: finding the Switch, walking objects, transferring files. The only module of the crate
//! with `unsafe` — COM calls.

#![allow(unsafe_code)]

use std::io::{Read, Write};
use std::marker::PhantomData;

use carafe_core::dbi::is_switch;
use carafe_core::ports::AdapterError;
use windows::Win32::Devices::PortableDevices::{
    IPortableDevice, IPortableDeviceContent, IPortableDeviceKeyCollection, IPortableDeviceManager,
    IPortableDeviceProperties, IPortableDeviceValues, PortableDeviceFTM,
    PortableDeviceKeyCollection, PortableDeviceManager, PortableDeviceValues,
    WPD_CLIENT_MAJOR_VERSION, WPD_CLIENT_NAME, WPD_CONTENT_TYPE_FOLDER,
    WPD_CONTENT_TYPE_FUNCTIONAL_OBJECT, WPD_CONTENT_TYPE_GENERIC_FILE, WPD_DEVICE_OBJECT_ID,
    WPD_OBJECT_CONTENT_TYPE, WPD_OBJECT_FORMAT, WPD_OBJECT_FORMAT_UNSPECIFIED, WPD_OBJECT_NAME,
    WPD_OBJECT_ORIGINAL_FILE_NAME, WPD_OBJECT_PARENT_ID, WPD_OBJECT_SIZE, WPD_RESOURCE_DEFAULT,
};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize, IStream, STGC_DEFAULT, STGM_READ,
};
use windows::core::{GUID, HSTRING, PCWSTR, PWSTR};

const CLIENT_NAME: &str = "Carafe";
const ENUM_BATCH: usize = 64;
const MIN_BUFFER: usize = 256 << 10;

/// COM on the calling thread: initialized on creation, released on drop.
pub struct Com {
    owned: bool,
}

impl Com {
    /// Initializes COM in multithreaded mode; if the thread is already single-threaded, works in that mode.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Device`] if COM fails to initialize.
    pub fn init() -> Result<Self, AdapterError> {
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result == RPC_E_CHANGED_MODE {
            return Ok(Self { owned: false });
        }
        result.ok().map_err(|error| device_error("COM", &error))?;
        Ok(Self { owned: true })
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.owned {
            unsafe { CoUninitialize() };
        }
    }
}

/// An object on the device.
#[derive(Debug, Clone)]
pub struct Object {
    /// The WPD object identifier.
    pub id: String,
    /// Name: for files, the original file name; for storages, their title.
    pub name: String,
    /// A folder or a storage rather than a file.
    pub is_folder: bool,
}

/// Returns the PnP identifier of the first connected Switch.
///
/// # Returns
///
/// `None` if no Switch is visible.
///
/// # Errors
///
/// [`AdapterError::Device`] if the device list cannot be obtained.
pub fn find_switch(_com: &Com) -> Result<Option<String>, AdapterError> {
    let manager: IPortableDeviceManager =
        unsafe { CoCreateInstance(&PortableDeviceManager, None, CLSCTX_INPROC_SERVER) }
            .map_err(|error| device_error("WPD manager", &error))?;
    let mut count = 0u32;
    unsafe { manager.GetDevices(std::ptr::null_mut(), &mut count) }
        .map_err(|error| device_error("WPD devices", &error))?;
    if count == 0 {
        return Ok(None);
    }
    let mut ids = vec![PWSTR::null(); count as usize];
    unsafe { manager.GetDevices(ids.as_mut_ptr(), &mut count) }
        .map_err(|error| device_error("WPD devices", &error))?;
    let ids: Vec<String> = ids
        .into_iter()
        .take(count as usize)
        .map(take_string)
        .collect();
    Ok(ids.into_iter().find(|id| is_switch(id)))
}

/// An open device.
pub struct Session<'com> {
    content: IPortableDeviceContent,
    properties: IPortableDeviceProperties,
    keys: IPortableDeviceKeyCollection,
    device: IPortableDevice,
    _com: PhantomData<&'com Com>,
}

impl<'com> Session<'com> {
    /// Opens the device by its PnP identifier.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Device`] if the device cannot be opened.
    pub fn open(_com: &'com Com, device_id: &str) -> Result<Self, AdapterError> {
        let open = || -> windows::core::Result<Self> {
            unsafe {
                let client: IPortableDeviceValues =
                    CoCreateInstance(&PortableDeviceValues, None, CLSCTX_INPROC_SERVER)?;
                client.SetStringValue(&WPD_CLIENT_NAME, &HSTRING::from(CLIENT_NAME))?;
                client.SetUnsignedIntegerValue(&WPD_CLIENT_MAJOR_VERSION, 1)?;
                let device: IPortableDevice =
                    CoCreateInstance(&PortableDeviceFTM, None, CLSCTX_INPROC_SERVER)?;
                device.Open(&HSTRING::from(device_id), &client)?;
                let content = device.Content()?;
                let properties = content.Properties()?;
                let keys: IPortableDeviceKeyCollection =
                    CoCreateInstance(&PortableDeviceKeyCollection, None, CLSCTX_INPROC_SERVER)?;
                keys.Add(&WPD_OBJECT_NAME)?;
                keys.Add(&WPD_OBJECT_ORIGINAL_FILE_NAME)?;
                keys.Add(&WPD_OBJECT_CONTENT_TYPE)?;
                Ok(Self {
                    content,
                    properties,
                    keys,
                    device,
                    _com: PhantomData,
                })
            }
        };
        open().map_err(|error| device_error("open Switch", &error))
    }

    /// Returns the storages of the device.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Device`] if the device did not respond.
    pub fn storages(&self) -> Result<Vec<Object>, AdapterError> {
        self.children_of(WPD_DEVICE_OBJECT_ID)
    }

    /// Returns the contents of the folder or storage `parent`.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Device`] if the device did not respond.
    pub fn children(&self, parent: &str) -> Result<Vec<Object>, AdapterError> {
        let parent = HSTRING::from(parent);
        self.children_of(PCWSTR(parent.as_ptr()))
    }

    fn children_of(&self, parent: PCWSTR) -> Result<Vec<Object>, AdapterError> {
        let list = || -> windows::core::Result<Vec<Object>> {
            let objects = unsafe {
                self.content
                    .EnumObjects(0, parent, None::<&IPortableDeviceValues>)?
            };
            let mut ids = Vec::new();
            loop {
                let mut batch = [PWSTR::null(); ENUM_BATCH];
                let mut fetched = 0u32;
                let result = unsafe { objects.Next(&mut batch, &mut fetched) };
                result.ok()?;
                ids.extend(batch.into_iter().take(fetched as usize).map(take_string));
                if (fetched as usize) < ENUM_BATCH {
                    break;
                }
            }
            ids.into_iter().map(|id| self.describe(id)).collect()
        };
        list().map_err(|error| device_error("list Switch folder", &error))
    }

    fn describe(&self, id: String) -> windows::core::Result<Object> {
        let wide = HSTRING::from(id.as_str());
        let values = unsafe { self.properties.GetValues(&wide, &self.keys)? };
        let name = unsafe { values.GetStringValue(&WPD_OBJECT_ORIGINAL_FILE_NAME) }
            .or_else(|_| unsafe { values.GetStringValue(&WPD_OBJECT_NAME) })
            .map(take_string)
            .unwrap_or_default();
        let kind =
            unsafe { values.GetGuidValue(&WPD_OBJECT_CONTENT_TYPE) }.unwrap_or(GUID::zeroed());
        Ok(Object {
            id,
            name,
            is_folder: kind == WPD_CONTENT_TYPE_FOLDER
                || kind == WPD_CONTENT_TYPE_FUNCTIONAL_OBJECT,
        })
    }

    /// Creates the file `name` of `size` bytes in the folder `parent` and writes `source` into it.
    ///
    /// `on_written` receives the number of written bytes after each chunk.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Device`] if the device refused; [`AdapterError::Io`] if `source` cannot be read.
    pub fn upload(
        &self,
        parent: &str,
        name: &str,
        size: u64,
        source: &mut dyn Read,
        on_written: &mut dyn FnMut(u64),
    ) -> Result<(), AdapterError> {
        let (stream, optimal) = self
            .create_file(parent, name, size)
            .map_err(|error| device_error("create file on Switch", &error))?;
        let mut buffer = vec![0; (optimal as usize).max(MIN_BUFFER)];
        let mut done = 0u64;
        loop {
            let read = source
                .read(&mut buffer)
                .map_err(|error| AdapterError::Io(error.to_string()))?;
            if read == 0 {
                break;
            }
            let mut offset = 0;
            while offset < read {
                let mut written = 0u32;
                let chunk = &buffer[offset..read];
                let result = unsafe {
                    stream.Write(
                        chunk.as_ptr().cast(),
                        u32::try_from(chunk.len()).unwrap_or(u32::MAX),
                        Some(&mut written),
                    )
                };
                result
                    .ok()
                    .map_err(|error| device_error("write to Switch", &error))?;
                if written == 0 {
                    return Err(AdapterError::Device("Switch accepted no bytes".to_owned()));
                }
                offset += written as usize;
            }
            done += read as u64;
            on_written(done);
        }
        unsafe { stream.Commit(STGC_DEFAULT) }
            .map_err(|error| device_error("finish file on Switch", &error))
    }

    fn create_file(
        &self,
        parent: &str,
        name: &str,
        size: u64,
    ) -> windows::core::Result<(IStream, u32)> {
        unsafe {
            let values: IPortableDeviceValues =
                CoCreateInstance(&PortableDeviceValues, None, CLSCTX_INPROC_SERVER)?;
            let name = HSTRING::from(name);
            values.SetStringValue(&WPD_OBJECT_PARENT_ID, &HSTRING::from(parent))?;
            values.SetUnsignedLargeIntegerValue(&WPD_OBJECT_SIZE, size)?;
            values.SetStringValue(&WPD_OBJECT_ORIGINAL_FILE_NAME, &name)?;
            values.SetStringValue(&WPD_OBJECT_NAME, &name)?;
            values.SetGuidValue(&WPD_OBJECT_CONTENT_TYPE, &WPD_CONTENT_TYPE_GENERIC_FILE)?;
            values.SetGuidValue(&WPD_OBJECT_FORMAT, &WPD_OBJECT_FORMAT_UNSPECIFIED)?;
            let mut stream = None;
            let mut optimal = 0u32;
            self.content.CreateObjectWithPropertiesAndData(
                &values,
                &mut stream,
                &mut optimal,
                std::ptr::null_mut(),
            )?;
            let stream = stream
                .ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_POINTER))?;
            Ok((stream, optimal))
        }
    }

    /// Reads the file `object` from the device into `target`.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Device`] if the device refused; [`AdapterError::Io`] if `target` cannot be written to.
    pub fn download(&self, object: &str, target: &mut dyn Write) -> Result<(), AdapterError> {
        let stream = unsafe {
            let resources = self
                .content
                .Transfer()
                .map_err(|error| device_error("read from Switch", &error))?;
            let mut optimal = 0u32;
            let mut stream = None;
            resources
                .GetStream(
                    &HSTRING::from(object),
                    &WPD_RESOURCE_DEFAULT,
                    STGM_READ.0,
                    &mut optimal,
                    &mut stream,
                )
                .map_err(|error| device_error("read from Switch", &error))?;
            stream.ok_or_else(|| AdapterError::Device("Switch returned no stream".to_owned()))?
        };
        let mut buffer = vec![0u8; MIN_BUFFER];
        loop {
            let mut read = 0u32;
            let result = unsafe {
                stream.Read(
                    buffer.as_mut_ptr().cast(),
                    u32::try_from(buffer.len()).unwrap_or(u32::MAX),
                    Some(&mut read),
                )
            };
            result
                .ok()
                .map_err(|error| device_error("read from Switch", &error))?;
            if read == 0 {
                break;
            }
            target
                .write_all(&buffer[..read as usize])
                .map_err(|error| AdapterError::Io(error.to_string()))?;
        }
        Ok(())
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        let _ = unsafe { self.device.Close() };
    }
}

fn take_string(value: PWSTR) -> String {
    if value.is_null() {
        return String::new();
    }
    let text = unsafe { value.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(value.0.cast_const().cast())) };
    text
}

fn device_error(action: &str, error: &windows::core::Error) -> AdapterError {
    AdapterError::Device(format!("{action}: {error}"))
}
