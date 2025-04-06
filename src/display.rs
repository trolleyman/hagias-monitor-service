use std::{
    collections::{HashMap, HashSet, hash_map},
    ffi::OsString,
    fmt,
    hash::{Hash, Hasher},
    os::windows::ffi::OsStringExt,
};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use unit_enum::UnitEnum;
use windows::{
    Wdk::Graphics::Direct3D::{
        D3DKMDT_VIDEO_SIGNAL_STANDARD, D3DKMDT_VSS_APPLE, D3DKMDT_VSS_EIA_861,
        D3DKMDT_VSS_EIA_861A, D3DKMDT_VSS_EIA_861B, D3DKMDT_VSS_IBM, D3DKMDT_VSS_NTSC_443,
        D3DKMDT_VSS_NTSC_J, D3DKMDT_VSS_NTSC_M, D3DKMDT_VSS_PAL_B, D3DKMDT_VSS_PAL_B1,
        D3DKMDT_VSS_PAL_D, D3DKMDT_VSS_PAL_G, D3DKMDT_VSS_PAL_H, D3DKMDT_VSS_PAL_I,
        D3DKMDT_VSS_PAL_K, D3DKMDT_VSS_PAL_K1, D3DKMDT_VSS_PAL_L, D3DKMDT_VSS_PAL_M,
        D3DKMDT_VSS_PAL_N, D3DKMDT_VSS_PAL_NC, D3DKMDT_VSS_SECAM_B, D3DKMDT_VSS_SECAM_D,
        D3DKMDT_VSS_SECAM_G, D3DKMDT_VSS_SECAM_H, D3DKMDT_VSS_SECAM_K, D3DKMDT_VSS_SECAM_K1,
        D3DKMDT_VSS_SECAM_L, D3DKMDT_VSS_SECAM_L1, D3DKMDT_VSS_UNINITIALIZED, D3DKMDT_VSS_VESA_CVT,
        D3DKMDT_VSS_VESA_DMT, D3DKMDT_VSS_VESA_GTF,
    },
    Win32::{
        Devices::Display::{
            DISPLAYCONFIG_2DREGION, DISPLAYCONFIG_ADAPTER_NAME,
            DISPLAYCONFIG_DEVICE_INFO_GET_ADAPTER_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
            DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_MODE_INFO_TYPE_DESKTOP_IMAGE,
            DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE, DISPLAYCONFIG_MODE_INFO_TYPE_TARGET,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_COMPONENT_VIDEO,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_COMPOSITE_VIDEO, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_D_JPN,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EXTERNAL,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_USB_TUNNEL,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DVI, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HD15,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HDMI, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_VIRTUAL,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_WIRED,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_MIRACAST, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SDI,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SDTVDONGLE, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SVIDEO,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED,
            DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EXTERNAL, DISPLAYCONFIG_PATH_INFO,
            DISPLAYCONFIG_PIXELFORMAT, DISPLAYCONFIG_PIXELFORMAT_8BPP,
            DISPLAYCONFIG_PIXELFORMAT_16BPP, DISPLAYCONFIG_PIXELFORMAT_24BPP,
            DISPLAYCONFIG_PIXELFORMAT_32BPP, DISPLAYCONFIG_PIXELFORMAT_NONGDI,
            DISPLAYCONFIG_RATIONAL, DISPLAYCONFIG_ROTATION, DISPLAYCONFIG_ROTATION_IDENTITY,
            DISPLAYCONFIG_ROTATION_ROTATE90, DISPLAYCONFIG_ROTATION_ROTATE180,
            DISPLAYCONFIG_ROTATION_ROTATE270, DISPLAYCONFIG_SCALING,
            DISPLAYCONFIG_SCALING_ASPECTRATIOCENTEREDMAX, DISPLAYCONFIG_SCALING_CENTERED,
            DISPLAYCONFIG_SCALING_CUSTOM, DISPLAYCONFIG_SCALING_IDENTITY,
            DISPLAYCONFIG_SCALING_PREFERRED, DISPLAYCONFIG_SCALING_STRETCHED,
            DISPLAYCONFIG_SCANLINE_ORDERING, DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED,
            DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED_LOWERFIELDFIRST,
            DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED_UPPERFIELDFIRST,
            DISPLAYCONFIG_SCANLINE_ORDERING_PROGRESSIVE,
            DISPLAYCONFIG_SCANLINE_ORDERING_UNSPECIFIED, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
            DISPLAYCONFIG_TARGET_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME_FLAGS,
            DISPLAYCONFIG_TOPOLOGY_ID, DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY,
            DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QDC_ALL_PATHS,
            QDC_DATABASE_CURRENT, QDC_ONLY_ACTIVE_PATHS, QUERY_DISPLAY_CONFIG_FLAGS,
            QueryDisplayConfig, SDC_APPLY, SDC_USE_SUPPLIED_DISPLAY_CONFIG, SetDisplayConfig,
        },
        Foundation::{
            ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, HLOCAL, LocalFree, POINTL, WIN32_ERROR,
        },
        Graphics::Gdi::{
            DISPLAYCONFIG_PATH_ACTIVE, DISPLAYCONFIG_PATH_CLONE_GROUP_INVALID,
            DISPLAYCONFIG_PATH_DESKTOP_IMAGE_IDX_INVALID, DISPLAYCONFIG_PATH_MODE_IDX_INVALID,
            DISPLAYCONFIG_PATH_SOURCE_MODE_IDX_INVALID, DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE,
            DISPLAYCONFIG_PATH_TARGET_MODE_IDX_INVALID, DISPLAYCONFIG_SOURCE_IN_USE,
            DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_BOOT,
            DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_PATH,
            DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_SYSTEM, DISPLAYCONFIG_TARGET_FORCIBLE,
            DISPLAYCONFIG_TARGET_IN_USE, DISPLAYCONFIG_TARGET_IS_HMD,
        },
        System::Diagnostics::Debug::{
            FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM,
            FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW,
        },
    },
    core::PWSTR,
};

pub fn windows_error_to_string(error: WIN32_ERROR) -> String {
    use winapi::um::winnt::LANG_NEUTRAL;
    use winapi::um::winnt::MAKELANGID;
    use winapi::um::winnt::SUBLANG_DEFAULT;

    let mut error_text: PWSTR = PWSTR(std::ptr::null_mut());
    unsafe {
        let num_chars = FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM
                | FORMAT_MESSAGE_ALLOCATE_BUFFER
                | FORMAT_MESSAGE_IGNORE_INSERTS,
            None,
            error.0,
            MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT).into(),
            PWSTR((&mut error_text) as *mut PWSTR as *mut _),
            0,
            None,
        );
        if num_chars == 0 {
            return format!("0x{:x}", error.0).into();
        }
        let string = OsString::from_wide(std::slice::from_raw_parts(error_text.0, num_chars as _));
        LocalFree(Some(HLOCAL(error_text.0 as *mut _)));
        format!("0x{:x} {}", error.0, string.display())
    }
}

struct DisplayConfigBuilder {
    source_modes: Vec<DisplaySourceMode>,
    target_modes: Vec<DisplayTargetMode>,
    paths: Vec<DisplayPath>,
    windows_display_source_mode_to_index: HashMap<u32, usize>,
    windows_display_target_mode_to_index: HashMap<u32, usize>,
    target_devices: HashMap<IdAndAdapterId, DisplayTargetDevice>,
}
impl DisplayConfigBuilder {
    pub fn new() -> Self {
        Self {
            source_modes: Vec::new(),
            target_modes: Vec::new(),
            paths: Vec::new(),
            windows_display_source_mode_to_index: HashMap::new(),
            windows_display_target_mode_to_index: HashMap::new(),
            target_devices: HashMap::new(),
        }
    }

    pub fn add_active_paths(
        &mut self,
        windows_display_config: &WindowsDisplayConfig,
    ) -> Result<()> {
        for path in &windows_display_config.paths {
            self.add_path_if_active(path, windows_display_config)?;
        }
        Ok(())
    }

    pub fn add_path_if_active(
        &mut self,
        path: &DISPLAYCONFIG_PATH_INFO,
        windows_display_config: &WindowsDisplayConfig,
    ) -> Result<Option<usize>> {
        if path.flags & DISPLAYCONFIG_PATH_ACTIVE == 0 {
            return Ok(None);
        }
        Ok(Some(self.add_path(path, windows_display_config)?))
    }

    pub fn add_path(
        &mut self,
        path: &DISPLAYCONFIG_PATH_INFO,
        windows_display_config: &WindowsDisplayConfig,
    ) -> Result<usize> {
        let source_mode_index = self.get_source_index_from_path(&path, windows_display_config)?;
        let target_mode_index = self.get_target_index_from_path(&path, windows_display_config)?;

        self.paths.push(DisplayPath {
            source: DisplayPathSource { source_mode_index },
            target: DisplayPathTarget {
                target_mode_index,
                output_technology: path.targetInfo.outputTechnology.into(),
                rotation: path.targetInfo.rotation.into(),
                scaling: path.targetInfo.scaling.into(),
                refresh_rate: path.targetInfo.refreshRate.into(),
                scanline_ordering: path.targetInfo.scanLineOrdering.into(),
            },
        });

        Ok(self.paths.len() - 1)
    }

    pub fn build(&self) -> DisplayConfig {
        DisplayConfig {
            source_modes: self.source_modes.clone(),
            target_modes: self.target_modes.clone(),
            paths: self.paths.clone(),
        }
    }

    fn get_source_mode_index(
        &mut self,
        windows_source_mode_index: u32,
        windows_display_config: &WindowsDisplayConfig,
    ) -> Result<usize> {
        match self
            .windows_display_source_mode_to_index
            .entry(windows_source_mode_index)
        {
            hash_map::Entry::Vacant(entry) => {
                let windows_mode_info = windows_display_config
                    .modes
                    .get(windows_source_mode_index as usize)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Source mode #{} not found", windows_source_mode_index)
                    })?;
                if windows_mode_info.infoType != DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE {
                    bail!(
                        "Mode #{} is not a source mode: {:?}",
                        windows_source_mode_index,
                        windows_mode_info.infoType
                    );
                }
                let windows_source_mode = unsafe { windows_mode_info.Anonymous.sourceMode };
                let source_mode = DisplaySourceMode {
                    width: windows_source_mode.width,
                    height: windows_source_mode.height,
                    pixel_format: windows_source_mode.pixelFormat.into(),
                    position: windows_source_mode.position.into(),
                };
                self.source_modes.push(source_mode);
                let index = self.source_modes.len() - 1;
                entry.insert(index);
                Ok(index)
            }
            hash_map::Entry::Occupied(entry) => Ok(*entry.get()),
        }
    }

    fn get_target_mode_index(
        &mut self,
        windows_target_mode_index: u32,
        windows_display_config: &WindowsDisplayConfig,
    ) -> Result<usize> {
        let windows_mode_info = windows_display_config
            .modes
            .get(windows_target_mode_index as usize)
            .ok_or_else(|| {
                anyhow::anyhow!("Target mode #{} not found", windows_target_mode_index)
            })?;
        if windows_mode_info.infoType != DISPLAYCONFIG_MODE_INFO_TYPE_TARGET {
            bail!(
                "Mode #{} is not a target mode: {:?}",
                windows_target_mode_index,
                windows_mode_info.infoType
            );
        }
        let device = self
            .get_target_device(windows_mode_info.id, windows_mode_info.adapterId)?
            .clone();
        match self
            .windows_display_target_mode_to_index
            .entry(windows_target_mode_index)
        {
            hash_map::Entry::Vacant(entry) => {
                let windows_target_mode = unsafe { windows_mode_info.Anonymous.targetMode };
                let (video_standard, v_sync_freq_divider) = if
                /* WINDOWS_VERSION >= 8.1 */
                true {
                    unsafe {
                        (
                            (windows_target_mode
                                .targetVideoSignalInfo
                                .Anonymous
                                .AdditionalSignalInfo
                                ._bitfield
                                & 0xFFFF) as i32,
                            ((windows_target_mode
                                .targetVideoSignalInfo
                                .Anonymous
                                .AdditionalSignalInfo
                                ._bitfield
                                >> 16)
                                & 0b111111) as u32,
                        )
                    }
                } else {
                    (0, 0)
                };
                let signal_info = windows_target_mode.targetVideoSignalInfo;
                let target_mode = DisplayTargetMode {
                    device: device.clone(),
                    pixel_rate: signal_info.pixelRate.into(),
                    h_sync_freq: signal_info.hSyncFreq.into(),
                    v_sync_freq: signal_info.vSyncFreq.into(),
                    active_size: signal_info.activeSize.into(),
                    total_size: signal_info.totalSize.into(),
                    video_standard: video_standard.into(),
                    v_sync_freq_divider,
                    scanline_ordering: signal_info.scanLineOrdering.into(),
                };
                self.target_modes.push(target_mode);
                let index = self.target_modes.len() - 1;
                entry.insert(index);
                Ok(index)
            }
            hash_map::Entry::Occupied(entry) => Ok(*entry.get()),
        }
    }

    fn get_source_index_from_path(
        &mut self,
        path: &DISPLAYCONFIG_PATH_INFO,
        windows_display_config: &WindowsDisplayConfig,
    ) -> Result<usize> {
        if path.flags & DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE
            == DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE
        {
            bail!("Virtual modes are not supported");
        }
        let windows_source_mode_index = unsafe { path.sourceInfo.Anonymous.modeInfoIdx };
        self.get_source_mode_index(windows_source_mode_index, windows_display_config)
    }

    fn get_target_index_from_path(
        &mut self,
        path: &DISPLAYCONFIG_PATH_INFO,
        windows_display_config: &WindowsDisplayConfig,
    ) -> Result<usize> {
        if path.flags & DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE
            == DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE
        {
            bail!("Virtual modes are not supported");
        }
        let windows_target_mode_index = unsafe { path.targetInfo.Anonymous.modeInfoIdx };
        self.get_target_mode_index(windows_target_mode_index, windows_display_config)
    }

    fn get_target_device<'a>(
        &'a mut self,
        id: u32,
        adapter_id: windows::Win32::Foundation::LUID,
    ) -> Result<&'a DisplayTargetDevice> {
        let id_and_adapter_id = IdAndAdapterId {
            id,
            adapter_id: adapter_id.into(),
        };
        match self.target_devices.entry(id_and_adapter_id) {
            hash_map::Entry::Vacant(entry) => {
                let target_device_name = get_target_device_name(id, adapter_id)?;
                let (edid_manufacture_id, edid_product_code_id) =
                    if is_target_device_edid_ids_valid(target_device_name.flags) {
                        (
                            Some(target_device_name.edidManufactureId),
                            Some(target_device_name.edidProductCodeId),
                        )
                    } else {
                        (None, None)
                    };
                let monitor_friendly_device_name = wchar_null_terminated_to_os_string(
                    &target_device_name.monitorFriendlyDeviceName,
                );
                let monitor_device_path =
                    wchar_null_terminated_to_os_string(&target_device_name.monitorDevicePath);
                let target_device = DisplayTargetDevice {
                    id,
                    adapter: Adapter::from_adapter_id(adapter_id)?,
                    output_technology: target_device_name.outputTechnology.into(),
                    edid_manufacture_id,
                    edid_product_code_id,
                    connector_instance: target_device_name.connectorInstance,
                    monitor_friendly_device_name: if monitor_friendly_device_name.len() > 0 {
                        Some(monitor_friendly_device_name)
                    } else {
                        None
                    },
                    monitor_device_path: if monitor_device_path.len() > 0 {
                        Some(monitor_device_path)
                    } else {
                        None
                    },
                };
                Ok(entry.insert(target_device))
            }
            hash_map::Entry::Occupied(entry) => Ok(entry.into_mut()),
        }
    }
}

/// All active display modes and paths, that can be serialized and restored later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub source_modes: Vec<DisplaySourceMode>,
    pub target_modes: Vec<DisplayTargetMode>,
    pub paths: Vec<DisplayPath>,
}

impl DisplayConfig {
    pub fn get() -> Result<Self> {
        let windows_display_config = WindowsDisplayConfig::get(DisplayQueryType::Active)?;
        Self::from_windows(&windows_display_config)
    }

    pub fn set(&self) -> Result<()> {
        let windows_display_config = self.to_windows()?;
        windows_display_config.set()
    }

    pub fn from_windows(windows_display_config: &WindowsDisplayConfig) -> Result<Self> {
        let mut builder = DisplayConfigBuilder::new();
        builder.add_active_paths(windows_display_config)?;
        Ok(builder.build())
    }

    pub fn to_windows(&self) -> Result<WindowsDisplayConfig> {
        // let windows_display_config = WindowsDisplayConfig::get(DisplayQueryType::All)?;
        // // TODO: Implement
        // Ok(windows_display_config)
        todo!()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adapter {
    #[serde(with = "crate::serde_override::os_string")]
    pub device_instance_path: OsString,
}
impl Adapter {
    pub fn from_adapter_id(
        adapter_id: impl Into<windows::Win32::Foundation::LUID>,
    ) -> Result<Self> {
        Ok(Self {
            device_instance_path: get_adapter_device_path(adapter_id.into())?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayTargetDevice {
    pub id: u32,
    pub adapter: Adapter,
    pub output_technology: OutputTechnology,
    pub edid_manufacture_id: Option<u16>,
    pub edid_product_code_id: Option<u16>,
    pub connector_instance: u32,
    #[serde(with = "crate::serde_override::option_os_string")]
    pub monitor_friendly_device_name: Option<OsString>,
    #[serde(with = "crate::serde_override::option_os_string")]
    pub monitor_device_path: Option<OsString>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayTargetMode {
    pub device: DisplayTargetDevice,
    pub pixel_rate: u64,
    pub h_sync_freq: Rational,
    pub v_sync_freq: Rational,
    pub active_size: Region,
    pub total_size: Region,
    pub video_standard: VideoStandard,
    pub v_sync_freq_divider: u32,
    pub scanline_ordering: ScanlineOrdering,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplaySourceMode {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub position: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayPath {
    pub source: DisplayPathSource,
    pub target: DisplayPathTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayPathSource {
    pub source_mode_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayPathTarget {
    pub target_mode_index: usize,
    pub output_technology: OutputTechnology,
    pub rotation: DisplayRotation,
    pub scaling: DisplayScaling,
    pub refresh_rate: Rational,
    pub scanline_ordering: ScanlineOrdering,
}

#[derive(Debug, Clone, Copy)]
pub enum DisplayQueryType {
    All,
    Active,
    Database,
}

impl DisplayQueryType {
    pub fn to_flags(self) -> QUERY_DISPLAY_CONFIG_FLAGS {
        match self {
            DisplayQueryType::All => QDC_ALL_PATHS,
            DisplayQueryType::Active => QDC_ONLY_ACTIVE_PATHS,
            DisplayQueryType::Database => QDC_DATABASE_CURRENT,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(transparent)]
pub struct LuidWrapper(windows::Win32::Foundation::LUID);
impl Hash for LuidWrapper {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.LowPart.hash(state);
        self.0.HighPart.hash(state);
    }
}
impl Eq for LuidWrapper {}
impl From<windows::Win32::Foundation::LUID> for LuidWrapper {
    fn from(luid: windows::Win32::Foundation::LUID) -> Self {
        LuidWrapper(luid)
    }
}
impl From<LuidWrapper> for windows::Win32::Foundation::LUID {
    fn from(luid: LuidWrapper) -> Self {
        luid.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct IdAndAdapterId {
    pub id: u32,
    pub adapter_id: LuidWrapper,
}

pub struct WindowsDisplayConfig {
    pub paths: Vec<DISPLAYCONFIG_PATH_INFO>,
    pub modes: Vec<DISPLAYCONFIG_MODE_INFO>,
    pub adapter_device_names: HashMap<LuidWrapper, OsString>,
    pub source_device_names: HashMap<IdAndAdapterId, DISPLAYCONFIG_SOURCE_DEVICE_NAME>,
    pub target_device_names: HashMap<IdAndAdapterId, DISPLAYCONFIG_TARGET_DEVICE_NAME>,
}

impl WindowsDisplayConfig {
    pub fn get(query: DisplayQueryType) -> Result<WindowsDisplayConfig> {
        let query_flags = query.to_flags();
        let mut paths = Vec::new();
        let mut modes = Vec::new();
        unsafe {
            loop {
                let mut num_paths = 0;
                let mut num_modes = 0;
                let result =
                    GetDisplayConfigBufferSizes(query_flags, &mut num_paths, &mut num_modes);
                if result != ERROR_SUCCESS {
                    bail!(
                        "GetDisplayConfigBufferSizes error: {}",
                        windows_error_to_string(result)
                    );
                }

                if paths.capacity() < num_paths as usize {
                    paths.reserve(num_paths as usize - paths.capacity());
                }

                if modes.capacity() < num_modes as usize {
                    modes.reserve(num_modes as usize - modes.capacity());
                }

                let mut current_topology_id = DISPLAYCONFIG_TOPOLOGY_ID(0);
                let result = QueryDisplayConfig(
                    query_flags,
                    &mut num_paths,
                    paths.as_mut_ptr(),
                    &mut num_modes,
                    modes.as_mut_ptr(),
                    if query_flags == QDC_DATABASE_CURRENT {
                        Some(&mut current_topology_id)
                    } else {
                        None
                    },
                );
                if result == ERROR_INSUFFICIENT_BUFFER {
                    continue;
                }
                if result != ERROR_SUCCESS {
                    bail!(
                        "QueryDisplayConfig error: {}",
                        windows_error_to_string(result)
                    );
                }

                paths.set_len(num_paths as usize);
                modes.set_len(num_modes as usize);

                let adapter_ids = modes
                    .iter()
                    .map(|m| m.adapterId.into())
                    .chain(paths.iter().flat_map(|path| {
                        [
                            path.sourceInfo.adapterId.into(),
                            path.targetInfo.adapterId.into(),
                        ]
                    }))
                    .collect::<HashSet<LuidWrapper>>();
                let ids_and_adapter_ids: HashSet<IdAndAdapterId> = modes
                    .iter()
                    .map(|m| IdAndAdapterId {
                        id: m.id,
                        adapter_id: m.adapterId.into(),
                    })
                    .chain(paths.iter().flat_map(|path| {
                        [
                            IdAndAdapterId {
                                id: path.sourceInfo.id,
                                adapter_id: path.sourceInfo.adapterId.into(),
                            },
                            IdAndAdapterId {
                                id: path.targetInfo.id,
                                adapter_id: path.targetInfo.adapterId.into(),
                            },
                        ]
                    }))
                    .collect();

                let mut adapter_device_names = HashMap::new();
                for adapter_id in adapter_ids {
                    match adapter_device_names.entry(adapter_id) {
                        hash_map::Entry::Vacant(entry) => {
                            entry.insert(get_adapter_device_path(adapter_id.into())?);
                        }
                        hash_map::Entry::Occupied(_) => {}
                    }
                }

                let mut source_device_names: HashMap<
                    IdAndAdapterId,
                    DISPLAYCONFIG_SOURCE_DEVICE_NAME,
                > = HashMap::new();
                for id_and_adapter_id in ids_and_adapter_ids.iter().copied() {
                    match source_device_names.entry(id_and_adapter_id) {
                        hash_map::Entry::Vacant(entry) => {
                            if let Ok(source_device_name) = get_source_device_name(
                                id_and_adapter_id.id,
                                id_and_adapter_id.adapter_id.into(),
                            ) {
                                entry.insert(source_device_name);
                            }
                        }
                        hash_map::Entry::Occupied(_) => {}
                    }
                }

                let mut target_device_names: HashMap<
                    IdAndAdapterId,
                    DISPLAYCONFIG_TARGET_DEVICE_NAME,
                > = HashMap::new();
                for id_and_adapter_id in ids_and_adapter_ids.iter().copied() {
                    match target_device_names.entry(id_and_adapter_id) {
                        hash_map::Entry::Vacant(entry) => {
                            if let Ok(target_device_name) = get_target_device_name(
                                id_and_adapter_id.id,
                                id_and_adapter_id.adapter_id.into(),
                            ) {
                                entry.insert(target_device_name);
                            }
                        }
                        hash_map::Entry::Occupied(_) => {}
                    }
                }

                return Ok(WindowsDisplayConfig {
                    paths,
                    modes,
                    adapter_device_names,
                    source_device_names,
                    target_device_names,
                });
            }
        }
    }

    pub fn set(&self) -> Result<()> {
        unsafe {
            let result = SetDisplayConfig(
                Some(&self.paths),
                Some(&self.modes),
                SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG,
            );
            if result as i64 != ERROR_SUCCESS.0 as i64 {
                bail!(
                    "SetDisplayConfig error: {}",
                    windows_error_to_string(WIN32_ERROR(result as u32))
                );
            }
        }
        Ok(())
    }

    pub fn print(&self) {
        for (i, mode) in self.modes.iter().enumerate() {
            self.print_mode(i, mode);
        }
        for (i, path) in self.paths.iter().enumerate() {
            self.print_path(i, path);
        }
    }

    fn format_adapter_id(&self, adapter_id: windows::Win32::Foundation::LUID) -> String {
        match self.adapter_device_names.get(&LuidWrapper(adapter_id)) {
            Some(name) => format!("{:?} {:?}", adapter_id, name),
            None => format!("{:?}", adapter_id),
        }
    }

    fn print_mode(&self, i: usize, mode: &DISPLAYCONFIG_MODE_INFO) {
        println!("Display Mode #{}", i);
        println!("  ID: {:?}", mode.id);
        println!("  Adapter ID: {}", self.format_adapter_id(mode.adapterId));
        println!("  Info Type: {:?}", mode.infoType);
        unsafe {
            match mode.infoType {
                DISPLAYCONFIG_MODE_INFO_TYPE_TARGET => {
                    let target_mode = mode.Anonymous.targetMode;
                    println!("  Target Mode:");
                    println!("    Video Signal Info:");
                    println!(
                        "      Pixel Rate: {}",
                        target_mode.targetVideoSignalInfo.pixelRate
                    );
                    println!(
                        "      HSync Freq: {}",
                        format_rational_frequency(target_mode.targetVideoSignalInfo.hSyncFreq)
                    );
                    println!(
                        "      VSync Freq: {}",
                        format_rational_frequency(target_mode.targetVideoSignalInfo.vSyncFreq)
                    );
                    println!(
                        "      Active Size: {:?}",
                        target_mode.targetVideoSignalInfo.activeSize
                    );
                    println!(
                        "      Total Size: {:?}",
                        target_mode.targetVideoSignalInfo.totalSize
                    );
                    println!(
                        "      Video Standard: {}",
                        target_mode.targetVideoSignalInfo.Anonymous.videoStandard
                    );
                    println!(
                        "      Scanline Ordering: {:?}",
                        target_mode.targetVideoSignalInfo.scanLineOrdering
                    );
                    self.print_target_device(&IdAndAdapterId {
                        id: mode.id,
                        adapter_id: LuidWrapper(mode.adapterId),
                    });
                }
                DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE => {
                    let source_mode = mode.Anonymous.sourceMode;
                    println!("  Source Mode:");
                    println!("    Width: {}", source_mode.width);
                    println!("    Height: {}", source_mode.height);
                    println!("    Pixel Format: {:?}", source_mode.pixelFormat);
                    println!("    Position: {:?}", source_mode.position);
                    self.print_source_device(&IdAndAdapterId {
                        id: mode.id,
                        adapter_id: LuidWrapper(mode.adapterId),
                    });
                }
                DISPLAYCONFIG_MODE_INFO_TYPE_DESKTOP_IMAGE => {
                    let desktop_image_info = mode.Anonymous.desktopImageInfo;
                    println!("  Desktop Image Info:");
                    println!(
                        "    Path Source Size: {:?}",
                        desktop_image_info.PathSourceSize
                    );
                    println!(
                        "    Desktop Image Region: {:?}",
                        desktop_image_info.DesktopImageRegion
                    );
                    println!(
                        "    Desktop Image Clip: {:?}",
                        desktop_image_info.DesktopImageClip
                    );
                }
                _ => {
                    println!("  <Unknown Mode>");
                }
            }
        }
        println!();
    }

    fn print_path(&self, i: usize, path: &DISPLAYCONFIG_PATH_INFO) {
        println!("Display Path #{}", i);
        self.print_path_source(path);
        self.print_path_target(path);
        println!("  Flags: 0x{:x}", path.flags);
        if path.flags & DISPLAYCONFIG_PATH_ACTIVE != 0 {
            println!("    DISPLAYCONFIG_PATH_ACTIVE");
        }
        if path.flags & DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE != 0 {
            println!("    DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE");
        }
        println!();
    }

    fn print_path_source(&self, path: &DISPLAYCONFIG_PATH_INFO) {
        println!("  Source:");
        println!("    ID: {}", path.sourceInfo.id);
        println!(
            "    Adapter ID: {}",
            self.format_adapter_id(path.sourceInfo.adapterId)
        );
        unsafe {
            if path.flags & DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE != 0 {
                let clone_group_id =
                    (path.sourceInfo.Anonymous.Anonymous._bitfield & 0xffff0000) >> 16;
                if clone_group_id == DISPLAYCONFIG_PATH_CLONE_GROUP_INVALID {
                    println!("    Clone Group ID: Invalid");
                } else {
                    println!("    Clone Group ID: {}", clone_group_id);
                }
                let source_mode_info_idx =
                    path.sourceInfo.Anonymous.Anonymous._bitfield & 0x0000ffff;
                if source_mode_info_idx == DISPLAYCONFIG_PATH_SOURCE_MODE_IDX_INVALID {
                    println!("    Source Mode Info Index: Invalid");
                } else {
                    println!("    Source Mode Info Index: {}", source_mode_info_idx);
                }
            } else {
                if path.sourceInfo.Anonymous.modeInfoIdx == DISPLAYCONFIG_PATH_MODE_IDX_INVALID {
                    println!("    Mode Info Index: Invalid");
                } else {
                    println!(
                        "    Mode Info Index: {}",
                        path.sourceInfo.Anonymous.modeInfoIdx
                    );
                }
            }
        }
        println!("    Status Flags: 0x{:x}", path.sourceInfo.statusFlags);
        if path.sourceInfo.statusFlags & DISPLAYCONFIG_SOURCE_IN_USE != 0 {
            println!("      DISPLAYCONFIG_SOURCE_IN_USE");
        }
        self.print_source_device(&IdAndAdapterId {
            id: path.sourceInfo.id,
            adapter_id: LuidWrapper(path.sourceInfo.adapterId),
        });
    }

    fn print_source_device(&self, id_and_adapter_id: &IdAndAdapterId) {
        if let Some(source_device_name) = self.source_device_names.get(id_and_adapter_id) {
            println!("    Source Device:");
            println!(
                "      GDI Device Name: {:?}",
                wchar_null_terminated_to_os_string(&source_device_name.viewGdiDeviceName)
            );
        } else {
            println!("    Source Device: <Unknown>");
        }
    }

    fn print_path_target(&self, path: &DISPLAYCONFIG_PATH_INFO) {
        println!("  Target:");
        println!("    ID: {}", path.targetInfo.id);
        println!(
            "    Adapter ID: {}",
            self.format_adapter_id(path.targetInfo.adapterId)
        );
        unsafe {
            if path.flags & DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE != 0 {
                let desktop_mode_info_idx =
                    (path.targetInfo.Anonymous.Anonymous._bitfield & 0xffff0000) >> 16;
                if desktop_mode_info_idx == DISPLAYCONFIG_PATH_DESKTOP_IMAGE_IDX_INVALID {
                    println!("    Desktop Mode ID: Invalid");
                } else {
                    println!("    Desktop Mode ID: {}", desktop_mode_info_idx);
                }
                let target_mode_info_idx =
                    path.sourceInfo.Anonymous.Anonymous._bitfield & 0x0000ffff;
                if target_mode_info_idx == DISPLAYCONFIG_PATH_TARGET_MODE_IDX_INVALID {
                    println!("    Target Mode Info Index: Invalid");
                } else {
                    println!("    Target Mode Info Index: {}", target_mode_info_idx);
                }
            } else {
                if path.sourceInfo.Anonymous.modeInfoIdx == DISPLAYCONFIG_PATH_MODE_IDX_INVALID {
                    println!("    Mode Info Index: Invalid");
                } else {
                    println!(
                        "    Mode Info Index: {}",
                        path.sourceInfo.Anonymous.modeInfoIdx
                    );
                }
            }
        }
        println!(
            "    Output Technology: {}",
            format_output_technology(path.targetInfo.outputTechnology)
        );
        println!("    Rotation: {:?}", path.targetInfo.rotation);
        println!("    Scaling: {:?}", path.targetInfo.scaling);
        println!(
            "    Refresh Rate: {}",
            format_rational_frequency(path.targetInfo.refreshRate)
        );
        println!(
            "    Scanline Ordering: {:?}",
            path.targetInfo.scanLineOrdering
        );
        println!(
            "    Target Available: {}",
            path.targetInfo.targetAvailable.as_bool()
        );
        println!("    Status Flags: 0x{:x}", path.targetInfo.statusFlags);
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_IN_USE != 0 {
            println!("      DISPLAYCONFIG_TARGET_IN_USE");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCIBLE != 0 {
            println!("      DISPLAYCONFIG_TARGET_FORCIBLE");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_BOOT != 0 {
            println!("      DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_BOOT");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_PATH != 0 {
            println!("      DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_PATH");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_SYSTEM != 0 {
            println!("      DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_SYSTEM");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_IS_HMD != 0 {
            println!("      DISPLAYCONFIG_TARGET_IS_HMD");
        }
        self.print_target_device(&IdAndAdapterId {
            id: path.targetInfo.id,
            adapter_id: LuidWrapper(path.targetInfo.adapterId),
        });
    }

    fn print_target_device(&self, id_and_adapter_id: &IdAndAdapterId) {
        if let Some(target_device_name) = self.target_device_names.get(id_and_adapter_id) {
            println!("    Target Device:");
            println!("      Flags: 0x{:x}", unsafe {
                target_device_name.flags.Anonymous.value
            });
            if is_target_device_friendly_name_from_edid(target_device_name.flags) {
                println!("        Friendly Name From EDID");
            }
            if is_target_device_friendly_name_forced(target_device_name.flags) {
                println!("        Friendly Name Forced");
            }
            if is_target_device_edid_ids_valid(target_device_name.flags) {
                println!("        EDID IDs Valid");
            }
            println!(
                "      Output Technology: {}",
                format_output_technology(target_device_name.outputTechnology)
            );
            if is_target_device_edid_ids_valid(target_device_name.flags) {
                println!(
                    "      EDID Manufacture ID: 0x{:x}",
                    target_device_name.edidManufactureId
                );
                println!(
                    "      EDID Product Code ID: 0x{:x}",
                    target_device_name.edidProductCodeId
                );
            }
            println!(
                "      Connector Instance: {}",
                target_device_name.connectorInstance
            );
            println!(
                "      Monitor Friendly Device Name: {:?}",
                wchar_null_terminated_to_os_string(&target_device_name.monitorFriendlyDeviceName)
            );
            println!(
                "      Monitor Device Path: {:?}",
                wchar_null_terminated_to_os_string(&target_device_name.monitorDevicePath)
            );
        } else {
            println!("    Target Device: <Unknown>");
        }
    }
}

pub fn is_target_device_friendly_name_from_edid(
    flags: DISPLAYCONFIG_TARGET_DEVICE_NAME_FLAGS,
) -> bool {
    unsafe { flags.Anonymous.value & 0x1 != 0 }
}

pub fn is_target_device_friendly_name_forced(
    flags: DISPLAYCONFIG_TARGET_DEVICE_NAME_FLAGS,
) -> bool {
    unsafe { flags.Anonymous.value & 0x2 != 0 }
}

pub fn is_target_device_edid_ids_valid(flags: DISPLAYCONFIG_TARGET_DEVICE_NAME_FLAGS) -> bool {
    unsafe { flags.Anonymous.value & 0x4 != 0 }
}

pub fn wchar_null_terminated_to_os_string(wchar: &[u16]) -> OsString {
    let len = wchar.iter().position(|&c| c == 0).unwrap_or(wchar.len());
    OsString::from_wide(&wchar[..len])
}

pub fn get_adapter_device_path(adapter_id: windows::Win32::Foundation::LUID) -> Result<OsString> {
    let mut device_name = DISPLAYCONFIG_ADAPTER_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADAPTER_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_ADAPTER_NAME>()
                .try_into()
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to convert size of DISPLAYCONFIG_ADAPTER_NAME to u32: {}",
                        e
                    )
                })?,
            adapterId: adapter_id,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe {
        let result = DisplayConfigGetDeviceInfo(&mut device_name.header as *mut _);
        if result != ERROR_SUCCESS.0 as i32 {
            bail!(
                "DisplayConfigGetDeviceInfo error: {}",
                windows_error_to_string(WIN32_ERROR(result as u32))
            );
        }
    }
    Ok(wchar_null_terminated_to_os_string(
        &device_name.adapterDevicePath,
    ))
}

pub fn get_source_device_name(
    id: u32,
    adapter_id: windows::Win32::Foundation::LUID,
) -> Result<DISPLAYCONFIG_SOURCE_DEVICE_NAME> {
    let mut device_name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>()
                .try_into()
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to convert size of DISPLAYCONFIG_SOURCE_DEVICE_NAME to u32: {}",
                        e
                    )
                })?,
            adapterId: adapter_id,
            id,
        },
        ..Default::default()
    };
    unsafe {
        let result = DisplayConfigGetDeviceInfo(&mut device_name.header as *mut _);
        if result != ERROR_SUCCESS.0 as i32 {
            bail!(
                "DisplayConfigGetDeviceInfo error: {}",
                windows_error_to_string(WIN32_ERROR(result as u32))
            );
        }
    }
    Ok(device_name)
}

pub fn get_target_device_name(
    id: u32,
    adapter_id: windows::Win32::Foundation::LUID,
) -> Result<DISPLAYCONFIG_TARGET_DEVICE_NAME> {
    let mut device_name = DISPLAYCONFIG_TARGET_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>()
                .try_into()
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to convert size of DISPLAYCONFIG_TARGET_DEVICE_NAME to u32: {}",
                        e
                    )
                })?,
            adapterId: adapter_id,
            id,
        },
        ..Default::default()
    };
    unsafe {
        let result = DisplayConfigGetDeviceInfo(&mut device_name.header as *mut _);
        if result != ERROR_SUCCESS.0 as i32 {
            bail!(
                "DisplayConfigGetDeviceInfo error: {}",
                windows_error_to_string(WIN32_ERROR(result as u32))
            );
        }
    }
    Ok(device_name)
}

pub fn format_output_technology(
    output_technology: DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY,
) -> String {
    format!("{:?}", OutputTechnology::from(output_technology))
}

pub fn format_rational_frequency(rational: DISPLAYCONFIG_RATIONAL) -> String {
    if rational.Denominator == 0 {
        format!("{}/{}", rational.Numerator, rational.Denominator)
    } else if rational.Denominator == 1 {
        format!("{}Hz", rational.Numerator)
    } else {
        format!(
            "{}Hz ({}/{})",
            rational.Numerator as f64 / rational.Denominator as f64,
            rational.Numerator,
            rational.Denominator
        )
    }
}

/// The target's connector type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, UnitEnum, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
#[repr(i32)]
pub enum OutputTechnology {
    /// Indicates an HD15 (VGA) connector.
    Hd15 = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HD15.0,
    /// Indicates an S-video connector.
    SVideo = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SVIDEO.0,
    /// Indicates a composite video connector group.
    CompositeVideo = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_COMPOSITE_VIDEO.0,
    /// Indicates a component video connector group.
    ComponentVideo = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_COMPONENT_VIDEO.0,
    /// Indicates a Digital Video Interface (DVI) connector.
    Dvi = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DVI.0,
    /// Indicates a High-Definition Multimedia Interface (HDMI) connector.
    Hdmi = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HDMI.0,
    /// Indicates a Low Voltage Differential Swing (LVDS) connector.
    Lvds = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS.0,
    /// Indicates a Japanese D connector.
    Djpn = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_D_JPN.0,
    /// Indicates an SDI connector.
    Sdi = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SDI.0,
    /// Indicates an external display port, which is a display port that connects externally to a display device.
    DisplayPortExternal = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EXTERNAL.0,
    /// Indicates an embedded display port that connects internally to a display device.
    DisplayPortEmbedded = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED.0,
    /// Indicates an external Unified Display Interface (UDI), which is a UDI that connects externally to a display device.
    UdiExternal = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EXTERNAL.0,
    /// Indicates an embedded UDI that connects internally to a display device.
    UdiEmbedded = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED.0,
    /// Indicates a dongle cable that supports standard definition television (SDTV).
    SdtvDongle = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SDTVDONGLE.0,
    /// Indicates that the VidPN target is a Miracast wireless display device.
    ///
    /// Supported starting in Windows 8.1.
    Miracast = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_MIRACAST.0,
    /// Indicates an indirect wired connection.
    IndirectWired = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_WIRED.0,
    /// Indicates an indirect virtual connection.
    IndirectVirtual = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_VIRTUAL.0,
    /// Indicates a DisplayPort USB tunnel.
    DisplayPortUsbTunnel = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_USB_TUNNEL.0,
    /// Indicates that the video output device connects internally to a display device (for example, the internal connection in a laptop computer).
    Internal = DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL.0,
    #[unit_enum(other)]
    Other(i32),
}

impl From<DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY> for OutputTechnology {
    fn from(value: DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY) -> Self {
        OutputTechnology::from(value.0)
    }
}

impl From<OutputTechnology> for DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY {
    fn from(value: OutputTechnology) -> Self {
        DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY(value.into())
    }
}

impl From<i32> for OutputTechnology {
    fn from(value: i32) -> Self {
        OutputTechnology::from_discriminant(value)
    }
}

impl From<OutputTechnology> for i32 {
    fn from(value: OutputTechnology) -> Self {
        value.discriminant()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rational {
    numerator: u32,
    denominator: u32,
}

impl fmt::Debug for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rational({}/{})", self.numerator, self.denominator)
    }
}

impl From<DISPLAYCONFIG_RATIONAL> for Rational {
    fn from(rational: DISPLAYCONFIG_RATIONAL) -> Self {
        Self {
            numerator: rational.Numerator,
            denominator: rational.Denominator,
        }
    }
}

impl From<Rational> for DISPLAYCONFIG_RATIONAL {
    fn from(rational: Rational) -> Self {
        Self {
            Numerator: rational.numerator,
            Denominator: rational.denominator,
        }
    }
}

/// A point or an offset in a two-dimensional space
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub x: u32,
    pub y: u32,
}

impl From<DISPLAYCONFIG_2DREGION> for Region {
    fn from(value: DISPLAYCONFIG_2DREGION) -> Self {
        Self {
            x: value.cx,
            y: value.cy,
        }
    }
}

impl From<Region> for DISPLAYCONFIG_2DREGION {
    fn from(value: Region) -> Self {
        Self {
            cx: value.x,
            cy: value.y,
        }
    }
}

/// A point in a two-dimensional space
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl From<POINTL> for Point {
    fn from(value: POINTL) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

impl From<Point> for POINTL {
    fn from(value: Point) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

/// The clockwise rotation of the display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, UnitEnum, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
#[repr(i32)]
pub enum DisplayRotation {
    /// Indicates that rotation is 0 degrees—landscape mode.
    Identity = DISPLAYCONFIG_ROTATION_IDENTITY.0,
    /// Indicates that rotation is 90 degrees clockwise—portrait mode.
    Rotate90 = DISPLAYCONFIG_ROTATION_ROTATE90.0,
    /// Indicates that rotation is 180 degrees clockwise—inverted landscape mode.
    Rotate180 = DISPLAYCONFIG_ROTATION_ROTATE180.0,
    /// Indicates that rotation is 270 degrees clockwise—inverted portrait mode.
    Rotate270 = DISPLAYCONFIG_ROTATION_ROTATE270.0,
    #[unit_enum(other)]
    Unknown(i32),
}

impl From<DISPLAYCONFIG_ROTATION> for DisplayRotation {
    fn from(value: DISPLAYCONFIG_ROTATION) -> Self {
        DisplayRotation::from(value.0)
    }
}

impl From<DisplayRotation> for DISPLAYCONFIG_ROTATION {
    fn from(value: DisplayRotation) -> Self {
        DISPLAYCONFIG_ROTATION(value.into())
    }
}

impl From<i32> for DisplayRotation {
    fn from(value: i32) -> Self {
        DisplayRotation::from_discriminant(value)
    }
}

impl From<DisplayRotation> for i32 {
    fn from(value: DisplayRotation) -> Self {
        value.discriminant()
    }
}

// The scaling transformation applied to content displayed on a video present network (VidPN) present path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, UnitEnum, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
#[repr(i32)]
pub enum DisplayScaling {
    /// Indicates the identity transformation; the source content is presented with no change. This transformation is available only if the path's source mode has the same spatial resolution as the path's target mode.
    Identity = DISPLAYCONFIG_SCALING_IDENTITY.0,
    /// Indicates the centering transformation; the source content is presented unscaled, centered with respect to the spatial resolution of the target mode.
    Centered = DISPLAYCONFIG_SCALING_CENTERED.0,
    /// Indicates the content is scaled to fit the path's target.
    Stretched = DISPLAYCONFIG_SCALING_STRETCHED.0,
    /// Indicates the aspect-ratio centering transformation.
    AspectRatioCenteredMax = DISPLAYCONFIG_SCALING_ASPECTRATIOCENTEREDMAX.0,
    /// Indicates that the caller requests a custom scaling that the caller cannot describe with any of the other `DISPLAYCONFIG_SCALING_XXX` values. Only a hardware vendor's value-add application should use `DISPLAYCONFIG_SCALING_CUSTOM`, because the value-add application might require a private interface to the driver. The application can then use `DISPLAYCONFIG_SCALING_CUSTOM` to indicate additional context for the driver for the custom value on the specified path.
    Custom = DISPLAYCONFIG_SCALING_CUSTOM.0,
    /// Indicates that the caller does not have any preference for the scaling. The `SetDisplayConfig` function will use the scaling value that was last saved in the database for the path. If such a scaling value does not exist, `SetDisplayConfig` will use the default scaling for the computer. For example, stretched (`DISPLAYCONFIG_SCALING_STRETCHED`) for tablet computers and aspect-ratio centered (`DISPLAYCONFIG_SCALING_ASPECTRATIOCENTEREDMAX`) for non-tablet computers.
    Preferred = DISPLAYCONFIG_SCALING_PREFERRED.0,
    #[unit_enum(other)]
    Unknown(i32),
}

impl From<DISPLAYCONFIG_SCALING> for DisplayScaling {
    fn from(value: DISPLAYCONFIG_SCALING) -> Self {
        DisplayScaling::from(value.0)
    }
}

impl From<DisplayScaling> for DISPLAYCONFIG_SCALING {
    fn from(value: DisplayScaling) -> Self {
        DISPLAYCONFIG_SCALING(value.into())
    }
}

impl From<i32> for DisplayScaling {
    fn from(value: i32) -> Self {
        DisplayScaling::from_discriminant(value)
    }
}

impl From<DisplayScaling> for i32 {
    fn from(value: DisplayScaling) -> Self {
        value.discriminant()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, UnitEnum, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
#[repr(i32)]
pub enum VideoStandard {
    /// Uninitialized.
    Uninitialized = D3DKMDT_VSS_UNINITIALIZED.0,
    /// The Video Electronics Standards Association (VESA) Display Monitor Timing (DMT) standard.
    VesaDmt = D3DKMDT_VSS_VESA_DMT.0,
    /// The VESA Generalized Timing Formula (GTF) standard.
    VesaGtf = D3DKMDT_VSS_VESA_GTF.0,
    /// The VESA Coordinated Video Timing (CVT) standard.
    VesaCvt = D3DKMDT_VSS_VESA_CVT.0,
    /// The IBM standard.
    Ibm = D3DKMDT_VSS_IBM.0,
    /// The Apple standard.
    Apple = D3DKMDT_VSS_APPLE.0,
    /// The National Television Standards Committee (NTSC) standard.
    NtscM = D3DKMDT_VSS_NTSC_M.0,
    /// The NTSC standard.
    NtscJ = D3DKMDT_VSS_NTSC_J.0,
    /// The NTSC standard.
    Ntsc443 = D3DKMDT_VSS_NTSC_443.0,
    /// The Phase Alteration Line (PAL) standard.
    PalB = D3DKMDT_VSS_PAL_B.0,
    /// The PAL standard.
    PalB1 = D3DKMDT_VSS_PAL_B1.0,
    /// The PAL standard.
    PalG = D3DKMDT_VSS_PAL_G.0,
    /// The PAL standard.
    PalH = D3DKMDT_VSS_PAL_H.0,
    /// The PAL standard.
    PalI = D3DKMDT_VSS_PAL_I.0,
    /// The PAL standard.
    PalD = D3DKMDT_VSS_PAL_D.0,
    /// The PAL standard.
    PalN = D3DKMDT_VSS_PAL_N.0,
    /// The PAL standard.
    PalNC = D3DKMDT_VSS_PAL_NC.0,
    /// The Systeme Electronic Pour Couleur Avec Memoire (SECAM) standard.
    SecamB = D3DKMDT_VSS_SECAM_B.0,
    /// The SECAM standard.
    SecamD = D3DKMDT_VSS_SECAM_D.0,
    /// The SECAM standard.
    SecamG = D3DKMDT_VSS_SECAM_G.0,
    /// The SECAM standard.
    SecamH = D3DKMDT_VSS_SECAM_H.0,
    /// The SECAM standard.
    SecamK = D3DKMDT_VSS_SECAM_K.0,
    /// The SECAM standard.
    SecamK1 = D3DKMDT_VSS_SECAM_K1.0,
    /// The SECAM standard.
    SecamL = D3DKMDT_VSS_SECAM_L.0,
    /// The SECAM standard.
    SecamL1 = D3DKMDT_VSS_SECAM_L1.0,
    /// The Electronics Industries Association (EIA) standard.
    Eia861 = D3DKMDT_VSS_EIA_861.0,
    /// The EIA standard.
    Eia861A = D3DKMDT_VSS_EIA_861A.0,
    /// The EIA standard.
    Eia861B = D3DKMDT_VSS_EIA_861B.0,
    /// The PAL standard.
    PalK = D3DKMDT_VSS_PAL_K.0,
    /// The PAL standard.
    PalK1 = D3DKMDT_VSS_PAL_K1.0,
    /// The PAL standard.
    PalL = D3DKMDT_VSS_PAL_L.0,
    /// The PAL standard.
    PalM = D3DKMDT_VSS_PAL_M.0,
    /// Any video standard other than those represented by the previous constants in this enumeration.
    #[unit_enum(other)]
    Other(i32),
}

impl From<D3DKMDT_VIDEO_SIGNAL_STANDARD> for VideoStandard {
    fn from(value: D3DKMDT_VIDEO_SIGNAL_STANDARD) -> Self {
        VideoStandard::from(value.0)
    }
}

impl From<VideoStandard> for D3DKMDT_VIDEO_SIGNAL_STANDARD {
    fn from(value: VideoStandard) -> Self {
        D3DKMDT_VIDEO_SIGNAL_STANDARD(value.into())
    }
}

impl From<i32> for VideoStandard {
    fn from(value: i32) -> Self {
        VideoStandard::from_discriminant(value)
    }
}

impl From<VideoStandard> for i32 {
    fn from(value: VideoStandard) -> Self {
        value.discriminant()
    }
}

/// The method that the display uses to create an image on a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, UnitEnum, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
#[repr(i32)]
pub enum ScanlineOrdering {
    /// Scan-line ordering of the output is unspecified.
    Unspecified = DISPLAYCONFIG_SCANLINE_ORDERING_UNSPECIFIED.0,
    /// Output is a progressive image.
    Progressive = DISPLAYCONFIG_SCANLINE_ORDERING_PROGRESSIVE.0,
    /// Output is an interlaced image with the upper field first.
    InterlacedUpperFieldFirst = DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED_UPPERFIELDFIRST.0,
    /// Output is an interlaced image with the lower field first.
    InterlacedLowerFieldFirst = DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED_LOWERFIELDFIRST.0,
    #[unit_enum(other)]
    Unknown(i32),
}

pub const INTERLACED: ScanlineOrdering = if DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED.0
    == DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED_UPPERFIELDFIRST.0
{
    ScanlineOrdering::InterlacedUpperFieldFirst
} else if DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED.0
    == DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED_LOWERFIELDFIRST.0
{
    ScanlineOrdering::InterlacedLowerFieldFirst
} else {
    ScanlineOrdering::Unknown(DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED.0)
};

impl From<DISPLAYCONFIG_SCANLINE_ORDERING> for ScanlineOrdering {
    fn from(value: DISPLAYCONFIG_SCANLINE_ORDERING) -> Self {
        ScanlineOrdering::from(value.0)
    }
}

impl From<ScanlineOrdering> for DISPLAYCONFIG_SCANLINE_ORDERING {
    fn from(value: ScanlineOrdering) -> Self {
        DISPLAYCONFIG_SCANLINE_ORDERING(value.into())
    }
}

impl From<i32> for ScanlineOrdering {
    fn from(value: i32) -> Self {
        ScanlineOrdering::from_discriminant(value)
    }
}

impl From<ScanlineOrdering> for i32 {
    fn from(value: ScanlineOrdering) -> Self {
        value.discriminant()
    }
}

/// The pixel format of the display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, UnitEnum, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
#[repr(i32)]
pub enum PixelFormat {
    /// 8 BPP format.
    Bpp8 = DISPLAYCONFIG_PIXELFORMAT_8BPP.0,
    /// 16 BPP format.
    Bpp16 = DISPLAYCONFIG_PIXELFORMAT_16BPP.0,
    /// 24 BPP format.
    Bpp24 = DISPLAYCONFIG_PIXELFORMAT_24BPP.0,
    /// 32 BPP format.
    Bpp32 = DISPLAYCONFIG_PIXELFORMAT_32BPP.0,
    /// Indicates that the current display is not an 8, 16, 24, or 32 BPP GDI desktop mode.
    Nongdi = DISPLAYCONFIG_PIXELFORMAT_NONGDI.0,
    #[unit_enum(other)]
    Unknown(i32),
}

impl From<DISPLAYCONFIG_PIXELFORMAT> for PixelFormat {
    fn from(value: DISPLAYCONFIG_PIXELFORMAT) -> Self {
        PixelFormat::from(value.0)
    }
}

impl From<PixelFormat> for DISPLAYCONFIG_PIXELFORMAT {
    fn from(value: PixelFormat) -> Self {
        DISPLAYCONFIG_PIXELFORMAT(value.into())
    }
}

impl From<i32> for PixelFormat {
    fn from(value: i32) -> Self {
        PixelFormat::from_discriminant(value)
    }
}

impl From<PixelFormat> for i32 {
    fn from(value: PixelFormat) -> Self {
        value.discriminant()
    }
}

/*
// Generic versions of Windows*, so that we can serialize to JSON and do other things

#[derive(Debug, Clone)]
pub struct DisplayConfig {
    paths: Vec<DisplayPath>,
    modes: Vec<DisplayMode>,
}

impl DisplayConfig {
    pub fn get(query: DisplayQueryType) -> Result<DisplayConfig> {
        let windows_display_config = WindowsDisplayConfig::get(query.into())?;
        Ok(windows_display_config.into())
    }
}

impl From<WindowsDisplayConfig> for DisplayConfig {
    fn from(_windows_display_config: WindowsDisplayConfig) -> Self {
        todo!()
    }
}

impl From<DisplayConfig> for WindowsDisplayConfig {
    fn from(_display_config: DisplayConfig) -> Self {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct DisplayPath {
    source_info: PathSourceInfo,
    target_info: PathTargetInfo,
    flags: PathFlags,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PathFlags: u32 {
        const ACTIVE = windows::Win32::Graphics::Gdi::DISPLAYCONFIG_PATH_ACTIVE;
        const SUPPORT_VIRTUAL_MODE = windows::Win32::Graphics::Gdi::DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE;
        const BOOST_REFRESH_RATE = 0x00000010;//windows::Win32::Graphics::Gdi::DISPLAYCONFIG_PATH_BOOST_REFRESH_RATE;
    }
}

#[derive(Debug, Clone)]
pub struct PathSourceInfo {
    adapter_id: Luid,
    id: u32,
    mode_info_idx: Option<usize>,
    clone_group_id: Option<usize>,
    status_flags: PathSourceStatusFlags,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PathSourceStatusFlags: u32 {
        const SOURCE_IN_USE = windows::Win32::Graphics::Gdi::DISPLAYCONFIG_SOURCE_IN_USE;
    }
}

/// Contains target information for a single path
#[derive(Debug, Clone)]
pub struct PathTargetInfo {
    /// The identifier of the adapter that the path is on.
    adapter_id: Luid,
    /// The target identifier on the specified adapter that this path relates to.
    id: u32,
    /// An index into the mode array that contains the target mode information for this path.
    mode_info_idx: Option<usize>,
    /// An index into the mode array that contains the desktop mode information for this path.
    desktop_mode_info_idx: Option<usize>,
    /// The target's connector type.
    output_technology: OutputTechnology,
    /// Rotation of the target.
    rotation: DisplayRotation,
    /// Source image target scaling.
    scaling: DisplayScaling,
    /// Refresh rate of the target.
    refresh_rate: Rational,
    /// Scanline ordering of the output on the target.
    scanline_ordering: ScanlineOrdering,
    /// Whether the target is available.
    target_available: bool,
    /// Status flags for the target.
    status_flags: PathTargetStatusFlags,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PathTargetStatusFlags: u32 {
        const IN_USE = windows::Win32::Graphics::Gdi::DISPLAYCONFIG_TARGET_IN_USE;
        const FORCIBLE = windows::Win32::Graphics::Gdi::DISPLAYCONFIG_TARGET_FORCIBLE;
        const FORCED_AVAILABILITY_BOOT = windows::Win32::Graphics::Gdi::DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_BOOT;
        const FORCED_AVAILABILITY_PATH = windows::Win32::Graphics::Gdi::DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_PATH;
        const FORCED_AVAILABILITY_SYSTEM = windows::Win32::Graphics::Gdi::DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_SYSTEM;
        const IS_HMD = windows::Win32::Graphics::Gdi::DISPLAYCONFIG_TARGET_IS_HMD;
    }
}
*/
