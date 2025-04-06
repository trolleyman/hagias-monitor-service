use std::{
    collections::{HashMap, HashSet, hash_map},
    ffi::OsString,
    hash::{Hash, Hasher},
    os::windows::ffi::OsStringExt,
};

use anyhow::{Result, bail};
use windows::{
    Win32::{
        Devices::Display::{
            DISPLAYCONFIG_ADAPTER_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_ADAPTER_NAME,
            DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO,
            DISPLAYCONFIG_MODE_INFO_TYPE_DESKTOP_IMAGE, DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE,
            DISPLAYCONFIG_MODE_INFO_TYPE_TARGET, DISPLAYCONFIG_PATH_INFO,
            DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME,
            DISPLAYCONFIG_TARGET_DEVICE_NAME_FLAGS, DISPLAYCONFIG_TOPOLOGY_ID,
            DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QDC_ALL_PATHS,
            QDC_DATABASE_CURRENT, QDC_ONLY_ACTIVE_PATHS, QUERY_DISPLAY_CONFIG_FLAGS,
            QueryDisplayConfig, SDC_APPLY, SDC_USE_SUPPLIED_DISPLAY_CONFIG, SetDisplayConfig,
        },
        Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, HLOCAL, LocalFree, WIN32_ERROR},
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
                            entry.insert(get_adapter_device_name(adapter_id.into())?);
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
        if path.flags & DISPLAYCONFIG_PATH_ACTIVE == DISPLAYCONFIG_PATH_ACTIVE {
            println!("    DISPLAYCONFIG_PATH_ACTIVE");
        }
        if path.flags & DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE
            == DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE
        {
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
            if path.flags & DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE
                == DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE
            {
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
        if path.sourceInfo.statusFlags & DISPLAYCONFIG_SOURCE_IN_USE == DISPLAYCONFIG_SOURCE_IN_USE
        {
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
            if path.flags & DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE
                == DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE
            {
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
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_IN_USE == DISPLAYCONFIG_TARGET_IN_USE
        {
            println!("      DISPLAYCONFIG_TARGET_IN_USE");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCIBLE
            == DISPLAYCONFIG_TARGET_FORCIBLE
        {
            println!("      DISPLAYCONFIG_TARGET_FORCIBLE");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_BOOT
            == DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_BOOT
        {
            println!("      DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_BOOT");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_PATH
            == DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_PATH
        {
            println!("      DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_PATH");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_SYSTEM
            == DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_SYSTEM
        {
            println!("      DISPLAYCONFIG_TARGET_FORCED_AVAILABILITY_SYSTEM");
        }
        if path.targetInfo.statusFlags & DISPLAYCONFIG_TARGET_IS_HMD == DISPLAYCONFIG_TARGET_IS_HMD
        {
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
    unsafe { flags.Anonymous.value & 0x1 == 0x1 }
}

pub fn is_target_device_friendly_name_forced(
    flags: DISPLAYCONFIG_TARGET_DEVICE_NAME_FLAGS,
) -> bool {
    unsafe { flags.Anonymous.value & 0x2 == 0x2 }
}

pub fn is_target_device_edid_ids_valid(flags: DISPLAYCONFIG_TARGET_DEVICE_NAME_FLAGS) -> bool {
    unsafe { flags.Anonymous.value & 0x4 == 0x4 }
}

pub fn wchar_null_terminated_to_os_string(wchar: &[u16]) -> OsString {
    let len = wchar.iter().position(|&c| c == 0).unwrap_or(wchar.len());
    OsString::from_wide(&wchar[..len])
}

pub fn get_adapter_device_name(adapter_id: windows::Win32::Foundation::LUID) -> Result<OsString> {
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
    output_technology: windows::Win32::Devices::Display::DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY,
) -> String {
    match output_technology {
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HD15 => "HD15".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SVIDEO => "SVideo".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_COMPOSITE_VIDEO => "Composite Video".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_COMPONENT_VIDEO => "Component Video".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DVI => "DVI".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HDMI => "HDMI".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS => "LVDS".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_D_JPN => "D-JPN".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SDI => "SDI".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EXTERNAL => "DisplayPort External".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED => "DisplayPort Embedded".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EXTERNAL => "UDI External".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED => "UDI Embedded".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SDTVDONGLE => "SDTV Dongle".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_MIRACAST => "Miracast".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_WIRED => "Indirect Wired".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_VIRTUAL => "Indirect Virtual".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_USB_TUNNEL => "DisplayPort USB Tunnel".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL => "Internal".to_string(),
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_OTHER => "Other".to_string(),
        _ => format!("Unknown ({})", output_technology.0),
    }
}

pub fn format_rational_frequency(
    rational: windows::Win32::Devices::Display::DISPLAYCONFIG_RATIONAL,
) -> String {
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

/*
// Generic versions of Windows*, so that we can serialize to JSON and do other things

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Luid {
    low: u32,
    high: i32,
}

impl From<windows::Win32::Foundation::LUID> for Luid {
    fn from(luid: windows::Win32::Foundation::LUID) -> Self {
        Self {
            low: luid.LowPart,
            high: luid.HighPart,
        }
    }
}

impl From<Luid> for windows::Win32::Foundation::LUID {
    fn from(luid: Luid) -> Self {
        Self {
            LowPart: luid.low,
            HighPart: luid.high,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rational {
    numerator: u32,
    denominator: u32,
}

impl fmt::Debug for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rational{{{}/{}}}", self.numerator, self.denominator)
    }
}

impl From<windows::Win32::Devices::Display::DISPLAYCONFIG_RATIONAL> for Rational {
    fn from(rational: windows::Win32::Devices::Display::DISPLAYCONFIG_RATIONAL) -> Self {
        Self {
            numerator: rational.Numerator,
            denominator: rational.Denominator,
        }
    }
}

impl From<Rational> for windows::Win32::Devices::Display::DISPLAYCONFIG_RATIONAL {
    fn from(rational: Rational) -> Self {
        Self {
            Numerator: rational.numerator,
            Denominator: rational.denominator,
        }
    }
}

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

/// The target's connector type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, UnitEnum)]
#[repr(i32)]
pub enum OutputTechnology {
    /// Indicates an HD15 (VGA) connector.
    Hd15 = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HD15.0,
    /// Indicates an S-video connector.
    SVideo = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SVIDEO.0,
    /// Indicates a composite video connector group.
    CompositeVideo =
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_COMPOSITE_VIDEO.0,
    /// Indicates a component video connector group.
    ComponentVideo =
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_COMPONENT_VIDEO.0,
    /// Indicates a Digital Video Interface (DVI) connector.
    Dvi = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DVI.0,
    /// Indicates a High-Definition Multimedia Interface (HDMI) connector.
    Hdmi = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HDMI.0,
    /// Indicates a Low Voltage Differential Swing (LVDS) connector.
    Lvds = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS.0,
    /// Indicates a Japanese D connector.
    Djpn = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_D_JPN.0,
    /// Indicates an SDI connector.
    Sdi = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SDI.0,
    /// Indicates an external display port, which is a display port that connects externally to a display device.
    DisplayPortExternal =
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EXTERNAL.0,
    /// Indicates an embedded display port that connects internally to a display device.
    DisplayPortEmbedded =
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED.0,
    /// Indicates an external Unified Display Interface (UDI), which is a UDI that connects externally to a display device.
    UdiExternal = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EXTERNAL.0,
    /// Indicates an embedded UDI that connects internally to a display device.
    UdiEmbedded = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED.0,
    /// Indicates a dongle cable that supports standard definition television (SDTV).
    SdtvDongle = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_SDTVDONGLE.0,
    /// Indicates that the VidPN target is a Miracast wireless display device.
    ///
    /// Supported starting in Windows 8.1.
    Miracast = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_MIRACAST.0,
    /// Indicates an indirect wired connection.
    IndirectWired =
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_WIRED.0,
    /// Indicates an indirect virtual connection.
    IndirectVirtual =
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_VIRTUAL.0,
    /// Indicates a DisplayPort USB tunnel.
    DisplayPortUsbTunnel =
        windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_USB_TUNNEL.0,
    /// Indicates that the video output device connects internally to a display device (for example, the internal connection in a laptop computer).
    Internal = windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL.0,
    #[unit_enum(other)]
    Other(i32),
}

impl From<windows::Win32::Devices::Display::DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY>
    for OutputTechnology
{
    fn from(
        value: windows::Win32::Devices::Display::DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY,
    ) -> Self {
        OutputTechnology::from_discriminant(value.0)
    }
}

impl From<OutputTechnology>
    for windows::Win32::Devices::Display::DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY
{
    fn from(value: OutputTechnology) -> Self {
        windows::Win32::Devices::Display::DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY(
            value.discriminant(),
        )
    }
}

/// The clockwise rotation of the display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, UnitEnum)]
#[repr(i32)]
pub enum DisplayRotation {
    /// Indicates that rotation is 0 degrees—landscape mode.
    Identity = windows::Win32::Devices::Display::DISPLAYCONFIG_ROTATION_IDENTITY.0,
    /// Indicates that rotation is 90 degrees clockwise—portrait mode.
    Rotate90 = windows::Win32::Devices::Display::DISPLAYCONFIG_ROTATION_ROTATE90.0,
    /// Indicates that rotation is 180 degrees clockwise—inverted landscape mode.
    Rotate180 = windows::Win32::Devices::Display::DISPLAYCONFIG_ROTATION_ROTATE180.0,
    /// Indicates that rotation is 270 degrees clockwise—inverted portrait mode.
    Rotate270 = windows::Win32::Devices::Display::DISPLAYCONFIG_ROTATION_ROTATE270.0,
    #[unit_enum(other)]
    Unknown(i32),
}

impl From<windows::Win32::Devices::Display::DISPLAYCONFIG_ROTATION> for DisplayRotation {
    fn from(value: windows::Win32::Devices::Display::DISPLAYCONFIG_ROTATION) -> Self {
        DisplayRotation::from_discriminant(value.0)
    }
}

impl From<DisplayRotation> for windows::Win32::Devices::Display::DISPLAYCONFIG_ROTATION {
    fn from(value: DisplayRotation) -> Self {
        windows::Win32::Devices::Display::DISPLAYCONFIG_ROTATION(value.discriminant())
    }
}

// The scaling transformation applied to content displayed on a video present network (VidPN) present path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, UnitEnum)]
#[repr(i32)]
pub enum DisplayScaling {
    /// Indicates the identity transformation; the source content is presented with no change. This transformation is available only if the path's source mode has the same spatial resolution as the path's target mode.
    Identity = windows::Win32::Devices::Display::DISPLAYCONFIG_SCALING_IDENTITY.0,
    /// Indicates the centering transformation; the source content is presented unscaled, centered with respect to the spatial resolution of the target mode.
    Centered = windows::Win32::Devices::Display::DISPLAYCONFIG_SCALING_CENTERED.0,
    /// Indicates the content is scaled to fit the path's target.
    Stretched = windows::Win32::Devices::Display::DISPLAYCONFIG_SCALING_STRETCHED.0,
    /// Indicates the aspect-ratio centering transformation.
    AspectRatioCenteredMax =
        windows::Win32::Devices::Display::DISPLAYCONFIG_SCALING_ASPECTRATIOCENTEREDMAX.0,
    /// Indicates that the caller requests a custom scaling that the caller cannot describe with any of the other `DISPLAYCONFIG_SCALING_XXX` values. Only a hardware vendor's value-add application should use `DISPLAYCONFIG_SCALING_CUSTOM`, because the value-add application might require a private interface to the driver. The application can then use `DISPLAYCONFIG_SCALING_CUSTOM` to indicate additional context for the driver for the custom value on the specified path.
    Custom = windows::Win32::Devices::Display::DISPLAYCONFIG_SCALING_CUSTOM.0,
    /// Indicates that the caller does not have any preference for the scaling. The `SetDisplayConfig` function will use the scaling value that was last saved in the database for the path. If such a scaling value does not exist, `SetDisplayConfig` will use the default scaling for the computer. For example, stretched (`DISPLAYCONFIG_SCALING_STRETCHED`) for tablet computers and aspect-ratio centered (`DISPLAYCONFIG_SCALING_ASPECTRATIOCENTEREDMAX`) for non-tablet computers.
    Preferred = windows::Win32::Devices::Display::DISPLAYCONFIG_SCALING_PREFERRED.0,
    #[unit_enum(other)]
    Unknown(i32),
}

impl From<windows::Win32::Devices::Display::DISPLAYCONFIG_SCALING> for DisplayScaling {
    fn from(value: windows::Win32::Devices::Display::DISPLAYCONFIG_SCALING) -> Self {
        DisplayScaling::from_discriminant(value.0)
    }
}

impl From<DisplayScaling> for windows::Win32::Devices::Display::DISPLAYCONFIG_SCALING {
    fn from(value: DisplayScaling) -> Self {
        windows::Win32::Devices::Display::DISPLAYCONFIG_SCALING(value.discriminant())
    }
}

/// The method that the display uses to create an image on a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, UnitEnum)]
#[repr(i32)]
pub enum ScanlineOrdering {
    /// Scan-line ordering of the output is unspecified.
    Unspecified = windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING_UNSPECIFIED.0,
    /// Output is a progressive image.
    Progressive = windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING_PROGRESSIVE.0,
    /// Output is an interlaced image with the upper field first.
    InterlacedUpperFieldFirst = windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED_UPPERFIELDFIRST.0,
    /// Output is an interlaced image with the lower field first.
    InterlacedLowerFieldFirst = windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED_LOWERFIELDFIRST.0,
    #[unit_enum(other)]
    Unknown(i32),
}

pub const INTERLACED: ScanlineOrdering = if windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED.0 == windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED_UPPERFIELDFIRST.0{
    ScanlineOrdering::InterlacedUpperFieldFirst
} else if windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED.0 == windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED_LOWERFIELDFIRST.0 {
    ScanlineOrdering::InterlacedLowerFieldFirst
} else {
    ScanlineOrdering::Unknown(windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING_INTERLACED.0)
};

impl From<windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING> for ScanlineOrdering {
    fn from(value: windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING) -> Self {
        ScanlineOrdering::from_discriminant(value.0)
    }
}

impl From<ScanlineOrdering> for windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING {
    fn from(value: ScanlineOrdering) -> Self {
        windows::Win32::Devices::Display::DISPLAYCONFIG_SCANLINE_ORDERING(value.discriminant())
    }
}
*/
