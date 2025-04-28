use std::{
    collections::{HashMap, HashSet, hash_map},
    ffi::OsString,
    fmt,
    hash::{Hash, Hasher},
    os::windows::ffi::OsStringExt,
};

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use tracing::debug;
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
            QueryDisplayConfig, SDC_APPLY, SDC_SAVE_TO_DATABASE, SDC_USE_SUPPLIED_DISPLAY_CONFIG,
            SetDisplayConfig,
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

use crate::display::DisplayTargetMode;

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

                return Ok(WindowsDisplayConfig::from_paths_and_modes(paths, modes)?);
            }
        }
    }

    pub fn from_paths_and_modes(
        paths: Vec<DISPLAYCONFIG_PATH_INFO>,
        modes: Vec<DISPLAYCONFIG_MODE_INFO>,
    ) -> Result<Self> {
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

        let mut source_device_names: HashMap<IdAndAdapterId, DISPLAYCONFIG_SOURCE_DEVICE_NAME> =
            HashMap::new();
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

        let mut target_device_names: HashMap<IdAndAdapterId, DISPLAYCONFIG_TARGET_DEVICE_NAME> =
            HashMap::new();
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

        Ok(Self {
            paths,
            modes,
            adapter_device_names,
            source_device_names,
            target_device_names,
        })
    }

    pub fn apply(&self, save_to_database: bool) -> Result<()> {
        unsafe {
            let mut flags = SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG;
            if save_to_database {
                flags |= SDC_SAVE_TO_DATABASE;
            }
            let result = SetDisplayConfig(Some(&self.paths), Some(&self.modes), flags);
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
        debug!("Display Mode #{}", i);
        debug!("  ID: {:?}", mode.id);
        debug!("  Adapter ID: {}", self.format_adapter_id(mode.adapterId));
        debug!("  Info Type: {:?}", mode.infoType);
        unsafe {
            match mode.infoType {
                DISPLAYCONFIG_MODE_INFO_TYPE_TARGET => {
                    let target_mode = mode.Anonymous.targetMode;
                    debug!("  Target Mode:");
                    debug!("    Video Signal Info:");
                    debug!(
                        "      Pixel Rate: {}",
                        target_mode.targetVideoSignalInfo.pixelRate
                    );
                    debug!(
                        "      HSync Freq: {}",
                        format_rational_frequency(target_mode.targetVideoSignalInfo.hSyncFreq)
                    );
                    debug!(
                        "      VSync Freq: {}",
                        format_rational_frequency(target_mode.targetVideoSignalInfo.vSyncFreq)
                    );
                    debug!(
                        "      Active Size: {:?}",
                        target_mode.targetVideoSignalInfo.activeSize
                    );
                    debug!(
                        "      Total Size: {:?}",
                        target_mode.targetVideoSignalInfo.totalSize
                    );
                    debug!(
                        "      Video Standard: {}",
                        target_mode.targetVideoSignalInfo.Anonymous.videoStandard
                    );
                    debug!(
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
                    debug!("  Source Mode:");
                    debug!("    Width: {}", source_mode.width);
                    debug!("    Height: {}", source_mode.height);
                    debug!("    Pixel Format: {:?}", source_mode.pixelFormat);
                    debug!("    Position: {:?}", source_mode.position);
                    self.print_source_device(&IdAndAdapterId {
                        id: mode.id,
                        adapter_id: LuidWrapper(mode.adapterId),
                    });
                }
                DISPLAYCONFIG_MODE_INFO_TYPE_DESKTOP_IMAGE => {
                    let desktop_image_info = mode.Anonymous.desktopImageInfo;
                    debug!("  Desktop Image Info:");
                    debug!(
                        "    Path Source Size: {:?}",
                        desktop_image_info.PathSourceSize
                    );
                    debug!(
                        "    Desktop Image Region: {:?}",
                        desktop_image_info.DesktopImageRegion
                    );
                    debug!(
                        "    Desktop Image Clip: {:?}",
                        desktop_image_info.DesktopImageClip
                    );
                }
                _ => {
                    debug!("  <Unknown Mode>");
                }
            }
        }
        debug!("");
    }

    fn print_path(&self, i: usize, path: &DISPLAYCONFIG_PATH_INFO) {
        debug!("Display Path #{}", i);
        self.print_path_source(path);
        self.print_path_target(path);
        debug!("  Flags: 0x{:x}", path.flags);
        if path.flags & DISPLAYCONFIG_PATH_ACTIVE != 0 {
            debug!("    DISPLAYCONFIG_PATH_ACTIVE");
        }
        if path.flags & DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE != 0 {
            debug!("    DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE");
        }
        debug!("");
    }

    fn print_path_source(&self, path: &DISPLAYCONFIG_PATH_INFO) {
        debug!("  Source:");
        debug!("    ID: {}", path.sourceInfo.id);
        debug!(
            "    Adapter ID: {}",
            self.format_adapter_id(path.sourceInfo.adapterId)
        );
        unsafe {
            if path.flags & DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE != 0 {
                let clone_group_id =
                    (path.sourceInfo.Anonymous.Anonymous._bitfield & 0xffff0000) >> 16;
                if clone_group_id == DISPLAYCONFIG_PATH_CLONE_GROUP_INVALID {
                    debug!("    Clone Group ID: Invalid");
                } else {
                    debug!("    Clone Group ID: {}", clone_group_id);
                }
                let source_mode_info_idx =
                    path.sourceInfo.Anonymous.Anonymous._bitfield & 0x0000ffff;
                if source_mode_info_idx == DISPLAYCONFIG_PATH_SOURCE_MODE_IDX_INVALID {
                    debug!("    Source Mode Info Index: Invalid");
                } else {
                    debug!("    Source Mode Info Index: {}", source_mode_info_idx);
                }
            } else {
                if path.sourceInfo.Anonymous.modeInfoIdx == DISPLAYCONFIG_PATH_MODE_IDX_INVALID {
                    debug!("    Mode Info Index: Invalid");
                } else {
                    debug!(
                        "    Mode Info Index: {}",
                        path.sourceInfo.Anonymous.modeInfoIdx
                    );
                }
            }
        }
        debug!("    Status Flags: 0x{:x}", path.sourceInfo.statusFlags);
        if path.sourceInfo.statusFlags & DISPLAYCONFIG_SOURCE_IN_USE != 0 {
            debug!("      DISPLAYCONFIG_SOURCE_IN_USE");
        }
        self.print_source_device(&IdAndAdapterId {
            id: path.sourceInfo.id,
            adapter_id: LuidWrapper(path.sourceInfo.adapterId),
        });
    }

    fn print_source_device(&self, id_and_adapter_id: &IdAndAdapterId) {
        if let Some(source_device_name) = self.source_device_names.get(id_and_adapter_id) {
            debug!("    Source Device:");
            debug!(
                "      GDI Device Name: {:?}",
                wchar_null_terminated_to_os_string(&source_device_name.viewGdiDeviceName)
            );
        } else {
            debug!("    Source Device: <Unknown>");
        }
    }

    fn print_path_target(&self, path: &DISPLAYCONFIG_PATH_INFO) {
        debug!("  Target:");
        debug!("    ID: {}", path.targetInfo.id);
        debug!(
            "    Adapter ID: {}",
            self.format_adapter_id(path.targetInfo.adapterId)
        );
        unsafe {
            if path.flags & DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE != 0 {
                let desktop_mode_info_idx =
                    (path.targetInfo.Anonymous.Anonymous._bitfield & 0xffff0000) >> 16;
                if desktop_mode_info_idx == DISPLAYCONFIG_PATH_DESKTOP_IMAGE_IDX_INVALID {
                    debug!("    Desktop Mode ID: Invalid");
                } else {
                    debug!("    Desktop Mode ID: {}", desktop_mode_info_idx);
                }
                let target_mode_info_idx =
                    path.sourceInfo.Anonymous.Anonymous._bitfield & 0x0000ffff;
                if target_mode_info_idx == DISPLAYCONFIG_PATH_TARGET_MODE_IDX_INVALID {
                    debug!("    Target Mode Info Index: Invalid");
                } else {
                    debug!("    Target Mode Info Index: {}", target_mode_info_idx);
                }
            } else {
                if path.sourceInfo.Anonymous.modeInfoIdx == DISPLAYCONFIG_PATH_MODE_IDX_INVALID {
                    debug!("    Mode Info Index: Invalid");
                } else {
                    debug!(
                        "    Mode Info Index: {}",
                        path.sourceInfo.Anonymous.modeInfoIdx
                    );
                }
            }
        }
        debug!(
            "    Output Technology: {}",
            format_output_technology(path.targetInfo.outputTechnology)
        );
        debug!("    Rotation: {:?}", path.targetInfo.rotation);
        debug!("    Scaling: {:?}", path.targetInfo.scaling);
        debug!(
            "    Refresh Rate: {}",
            format_rational_frequency(path.targetInfo.refreshRate)
        );
        debug!(
            "    Scanline Ordering: {:?}",
            path.targetInfo.scanLineOrdering
        );
        debug!(
            "    Target Available: {}",
            path.targetInfo.targetAvailable.as_bool()
        );
        debug!("    Status Flags: 0x{:x}", path.targetInfo.statusFlags);
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_IN_USE != 0 {
            debug!("      DISPLAYCONFIG_TARGET_IN_USE");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCIBLE != 0 {
            debug!("      DISPLAYCONFIG_TARGET_FORCIBLE");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_BOOT != 0 {
            debug!("      DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_BOOT");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_PATH != 0 {
            debug!("      DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_PATH");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_SYSTEM != 0 {
            debug!("      DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_SYSTEM");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_IS_HMD != 0 {
            debug!("      DISPLAYCONFIG_TARGET_IS_HMD");
        }
        self.print_target_device(&IdAndAdapterId {
            id: path.targetInfo.id,
            adapter_id: LuidWrapper(path.targetInfo.adapterId),
        });
    }

    fn print_target_device(&self, id_and_adapter_id: &IdAndAdapterId) {
        if let Some(target_device_name) = self.target_device_names.get(id_and_adapter_id) {
            debug!("    Target Device:");
            debug!("      Flags: 0x{:x}", unsafe {
                target_device_name.flags.Anonymous.value
            });
            if is_target_device_friendly_name_from_edid(target_device_name.flags) {
                debug!("        Friendly Name From EDID");
            }
            if is_target_device_friendly_name_forced(target_device_name.flags) {
                debug!("        Friendly Name Forced");
            }
            if is_target_device_edid_ids_valid(target_device_name.flags) {
                debug!("        EDID IDs Valid");
            }
            debug!(
                "      Output Technology: {}",
                format_output_technology(target_device_name.outputTechnology)
            );
            if is_target_device_edid_ids_valid(target_device_name.flags) {
                debug!(
                    "      EDID Manufacture ID: 0x{:x}",
                    target_device_name.edidManufactureId
                );
                debug!(
                    "      EDID Product Code ID: 0x{:x}",
                    target_device_name.edidProductCodeId
                );
            }
            debug!(
                "      Connector Instance: {}",
                target_device_name.connectorInstance
            );
            debug!(
                "      Monitor Friendly Device Name: {:?}",
                get_monitor_friendly_device_name(&target_device_name)
            );
            debug!(
                "      Monitor Device Path: {:?}",
                get_monitor_device_path(&target_device_name)
            );
        } else {
            debug!("    Target Device: <Unknown>");
        }
    }

    /// Get the best matching target mode for the given adapter ID and target mode
    ///
    /// Return error if no matching target mode is found
    pub fn get_matching_target_mode_id(
        &self,
        adapter_id: LuidWrapper,
        target_mode: &DisplayTargetMode,
    ) -> Result<u32> {
        let target_modes_with_matching_adapter_ids: Vec<_> = self
            .modes
            .iter()
            .filter(|mode| {
                mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_TARGET
                    && LuidWrapper::from(mode.adapterId) == adapter_id
            })
            .copied()
            .collect();

        let path_target_infos_with_matching_adapter_ids: Vec<_> = self
            .paths
            .iter()
            .map(|path| path.targetInfo)
            .filter(|target_info| LuidWrapper::from(target_info.adapterId) == adapter_id)
            .collect();

        let adapter_id_all_ids: HashSet<u32> = target_modes_with_matching_adapter_ids
            .iter()
            .map(|mode| mode.id)
            .chain(
                path_target_infos_with_matching_adapter_ids
                    .iter()
                    .map(|info| info.id),
            )
            .collect();

        let devices_by_id = adapter_id_all_ids
            .iter()
            .map(|&id| get_target_device_name(id, adapter_id.into()).map(|name| (id, name)))
            .collect::<Result<HashMap<u32, DISPLAYCONFIG_TARGET_DEVICE_NAME>>>()?;

        if let Some(target_mode_device_path) = &target_mode.device.monitor_device_path {
            let devices_with_matching_device_path: HashMap<u32, DISPLAYCONFIG_TARGET_DEVICE_NAME> =
                devices_by_id
                    .iter()
                    .map(|(&id, &device)| (id, device))
                    .filter(|(_, device)| {
                        get_monitor_device_path(device).as_ref() == Some(target_mode_device_path)
                    })
                    .collect();

            match devices_with_matching_device_path.len() {
                0 => {
                    // Fallback
                    debug!(
                        "No matching target mode found for device path, using fallback {}: {:?}",
                        target_mode.device.id, target_mode_device_path
                    );
                    Ok(target_mode.device.id)
                }
                1 => Ok(devices_with_matching_device_path
                    .into_iter()
                    .next()
                    .unwrap()
                    .0),
                _ => {
                    bail!(
                        "Multiple matching target modes found for device path: {:?}",
                        target_mode_device_path
                    );
                }
            }
        } else {
            bail!(
                "Not implemented: No device path found for target mode: {:?}",
                target_mode
            );
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

pub fn get_monitor_friendly_device_name(
    target_device_name: &DISPLAYCONFIG_TARGET_DEVICE_NAME,
) -> Option<OsString> {
    let monitor_friendly_device_name =
        wchar_null_terminated_to_os_string(&target_device_name.monitorFriendlyDeviceName);
    let monitor_friendly_device_name = if monitor_friendly_device_name.len() > 0 {
        Some(monitor_friendly_device_name)
    } else {
        None
    };
    monitor_friendly_device_name
}

pub fn get_monitor_device_path(
    target_device_name: &DISPLAYCONFIG_TARGET_DEVICE_NAME,
) -> Option<OsString> {
    let monitor_device_path =
        wchar_null_terminated_to_os_string(&target_device_name.monitorDevicePath);
    let monitor_device_path = if monitor_device_path.len() > 0 {
        Some(monitor_device_path)
    } else {
        None
    };
    monitor_device_path
}

pub fn get_adapter_device_path(adapter_id: windows::Win32::Foundation::LUID) -> Result<OsString> {
    let mut device_name = DISPLAYCONFIG_ADAPTER_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADAPTER_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_ADAPTER_NAME>()
                .try_into()
                .map_err(|e| {
                    anyhow!(
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
    adapter_id: LuidWrapper,
) -> Result<DISPLAYCONFIG_SOURCE_DEVICE_NAME> {
    let mut device_name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>()
                .try_into()
                .map_err(|e| {
                    anyhow!(
                        "Failed to convert size of DISPLAYCONFIG_SOURCE_DEVICE_NAME to u32: {}",
                        e
                    )
                })?,
            adapterId: adapter_id.into(),
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
                    anyhow!(
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
