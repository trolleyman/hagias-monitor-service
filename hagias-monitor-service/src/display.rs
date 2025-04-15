use std::{
    collections::{HashMap, hash_map},
    ffi::OsString,
};

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use windows::Win32::{
    Devices::Display::{
        DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_MODE_INFO_0, DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE,
        DISPLAYCONFIG_MODE_INFO_TYPE_TARGET, DISPLAYCONFIG_PATH_INFO,
        DISPLAYCONFIG_PATH_SOURCE_INFO, DISPLAYCONFIG_PATH_SOURCE_INFO_0,
        DISPLAYCONFIG_PATH_TARGET_INFO, DISPLAYCONFIG_PATH_TARGET_INFO_0,
        DISPLAYCONFIG_SOURCE_MODE, DISPLAYCONFIG_TARGET_MODE, DISPLAYCONFIG_VIDEO_SIGNAL_INFO,
        DISPLAYCONFIG_VIDEO_SIGNAL_INFO_0,
    },
    Graphics::Gdi::{
        DISPLAYCONFIG_PATH_ACTIVE, DISPLAYCONFIG_PATH_SUPPORT_VIRTUAL_MODE,
        DISPLAYCONFIG_SOURCE_IN_USE, DISPLAYCONFIG_TARGET_IN_USE,
    },
};

use crate::windows_util::{
    DisplayQueryType, DisplayRotation, DisplayScaling, IdAndAdapterId, LuidWrapper,
    OutputTechnology, PixelFormat, Point, Rational, Region, ScanlineOrdering, VideoStandard,
    WindowsDisplayConfig, get_adapter_device_path, get_monitor_device_path,
    get_monitor_friendly_device_name, get_source_device_name, get_target_device_name,
    is_target_device_edid_ids_valid, wchar_null_terminated_to_os_string,
};

struct DisplayConfigBuilder {
    source_modes: Vec<DisplaySourceMode>,
    target_modes: Vec<DisplayTargetMode>,
    paths: Vec<DisplayPath>,
    windows_display_source_mode_to_index: HashMap<u32, usize>,
    windows_display_target_mode_to_index: HashMap<u32, usize>,
    target_devices: HashMap<IdAndAdapterId, DisplayTargetDevice>,
    source_devices: HashMap<IdAndAdapterId, DisplaySourceDevice>,
    adapters: HashMap<LuidWrapper, Adapter>,
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
            source_devices: HashMap::new(),
            adapters: HashMap::new(),
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

    pub fn build(&self) -> DisplayLayout {
        DisplayLayout {
            source_modes: self.source_modes.clone(),
            target_modes: self.target_modes.clone(),
            paths: self.paths.clone(),
        }
    }

    fn get_adapter(&mut self, adapter_id: LuidWrapper) -> Result<&Adapter> {
        match self.adapters.entry(adapter_id) {
            hash_map::Entry::Vacant(entry) => {
                Ok(entry.insert(Adapter::from_adapter_id(adapter_id)?))
            }
            hash_map::Entry::Occupied(entry) => Ok(entry.into_mut()),
        }
    }

    fn get_source_mode_index(
        &mut self,
        windows_source_mode_index: u32,
        windows_display_config: &WindowsDisplayConfig,
    ) -> Result<usize> {
        if self
            .windows_display_source_mode_to_index
            .contains_key(&windows_source_mode_index)
        {
            return Ok(self.windows_display_source_mode_to_index[&windows_source_mode_index]);
        }

        let windows_mode_info = windows_display_config
            .modes
            .get(windows_source_mode_index as usize)
            .ok_or_else(|| anyhow!("Source mode #{} not found", windows_source_mode_index))?;
        if windows_mode_info.infoType != DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE {
            bail!(
                "Mode #{} is not a source mode: {:?}",
                windows_source_mode_index,
                windows_mode_info.infoType
            );
        }
        let windows_source_mode = unsafe { windows_mode_info.Anonymous.sourceMode };

        let device = self
            .get_source_device(windows_mode_info.id, windows_mode_info.adapterId.into())?
            .clone();

        let source_mode = DisplaySourceMode {
            device,
            width: windows_source_mode.width,
            height: windows_source_mode.height,
            pixel_format: windows_source_mode.pixelFormat.into(),
            position: windows_source_mode.position.into(),
        };
        self.source_modes.push(source_mode);
        let index = self.source_modes.len() - 1;
        self.windows_display_source_mode_to_index
            .insert(windows_source_mode_index, index);
        Ok(index)
    }

    fn get_target_mode_index(
        &mut self,
        windows_target_mode_index: u32,
        windows_display_config: &WindowsDisplayConfig,
    ) -> Result<usize> {
        if self
            .windows_display_target_mode_to_index
            .contains_key(&windows_target_mode_index)
        {
            return Ok(self.windows_display_target_mode_to_index[&windows_target_mode_index]);
        }
        let windows_mode_info = windows_display_config
            .modes
            .get(windows_target_mode_index as usize)
            .ok_or_else(|| anyhow!("Target mode #{} not found", windows_target_mode_index))?;
        if windows_mode_info.infoType != DISPLAYCONFIG_MODE_INFO_TYPE_TARGET {
            bail!(
                "Mode #{} is not a target mode: {:?}",
                windows_target_mode_index,
                windows_mode_info.infoType
            );
        }

        let device = self
            .get_target_device(windows_mode_info.id, windows_mode_info.adapterId.into())?
            .clone();

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
        self.windows_display_target_mode_to_index
            .insert(windows_target_mode_index, index);
        Ok(index)
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

    fn get_source_device(
        &mut self,
        id: u32,
        adapter_id: LuidWrapper,
    ) -> Result<&DisplaySourceDevice> {
        let id_and_adapter_id = IdAndAdapterId { id, adapter_id };
        if !self.source_devices.contains_key(&id_and_adapter_id) {
            let adapter = self.get_adapter(adapter_id)?.clone();

            let source_device_name = get_source_device_name(id, adapter_id)?;
            let source_device = DisplaySourceDevice {
                id,
                adapter,
                gdi_device_name: wchar_null_terminated_to_os_string(
                    &source_device_name.viewGdiDeviceName,
                ),
            };
            self.source_devices.insert(id_and_adapter_id, source_device);
        }
        Ok(&self.source_devices[&id_and_adapter_id])
    }

    fn get_target_device(
        &mut self,
        id: u32,
        adapter_id: LuidWrapper,
    ) -> Result<&DisplayTargetDevice> {
        let id_and_adapter_id = IdAndAdapterId { id, adapter_id };
        if !self.target_devices.contains_key(&id_and_adapter_id) {
            let adapter = self.get_adapter(adapter_id)?.clone();

            let target_device_name = get_target_device_name(id, adapter_id.into())?;
            let (edid_manufacture_id, edid_product_code_id) =
                if is_target_device_edid_ids_valid(target_device_name.flags) {
                    (
                        Some(target_device_name.edidManufactureId),
                        Some(target_device_name.edidProductCodeId),
                    )
                } else {
                    (None, None)
                };
            let monitor_friendly_device_name =
                get_monitor_friendly_device_name(&target_device_name);
            let monitor_device_path = get_monitor_device_path(&target_device_name);
            let target_device = DisplayTargetDevice {
                id,
                adapter,
                output_technology: target_device_name.outputTechnology.into(),
                edid_manufacture_id,
                edid_product_code_id,
                connector_instance: target_device_name.connectorInstance,
                monitor_friendly_device_name,
                monitor_device_path,
            };
            self.target_devices.insert(id_and_adapter_id, target_device);
        }
        Ok(&self.target_devices[&id_and_adapter_id])
    }
}

/// All active display modes and paths, that can be serialized and restored later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayLayout {
    pub source_modes: Vec<DisplaySourceMode>,
    pub target_modes: Vec<DisplayTargetMode>,
    pub paths: Vec<DisplayPath>,
}

impl DisplayLayout {
    pub fn get() -> Result<Self> {
        let windows_display_config = WindowsDisplayConfig::get(DisplayQueryType::Active)?;
        Self::from_windows(&windows_display_config)
    }

    pub fn apply(&self) -> Result<()> {
        let windows_display_config = self.to_windows()?;
        // println!("=== New display config ===");
        // windows_display_config.print();
        // println!("=== End of new display config ===");
        windows_display_config.apply()
    }

    pub fn from_windows(windows_display_config: &WindowsDisplayConfig) -> Result<Self> {
        let mut builder = DisplayConfigBuilder::new();
        builder.add_active_paths(windows_display_config)?;
        Ok(builder.build())
    }

    pub fn to_windows(&self) -> Result<WindowsDisplayConfig> {
        let windows_display_config = WindowsDisplayConfig::get(DisplayQueryType::All)?;
        // println!("=== Existing display config ===");
        // windows_display_config.print();
        // println!("=== End of existing display config ===");

        let mut new_windows_modes = Vec::new();
        let mut new_windows_paths = Vec::new();

        // Get device path => adapter IDs
        let device_path_to_adapter_id = windows_display_config
            .adapter_device_names
            .iter()
            .map(|(adapter_id, device_path)| (device_path.clone(), adapter_id.clone()))
            .collect::<HashMap<OsString, LuidWrapper>>();

        // Populate source modes
        for source_mode in self.source_modes.iter() {
            let adapter_id = *device_path_to_adapter_id
                .get(&source_mode.device.adapter.device_instance_path)
                .ok_or_else(|| {
                    anyhow!(
                        "Adapter ID not found for device path: {:?}",
                        source_mode.device.adapter.device_instance_path
                    )
                })?;

            // TODO: Map GDI device name instead of direct ID

            let windows_source_mode = DISPLAYCONFIG_MODE_INFO {
                id: source_mode.device.id,
                adapterId: adapter_id.into(),
                infoType: DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE,
                Anonymous: DISPLAYCONFIG_MODE_INFO_0 {
                    sourceMode: DISPLAYCONFIG_SOURCE_MODE {
                        width: source_mode.width,
                        height: source_mode.height,
                        pixelFormat: source_mode.pixel_format.into(),
                        position: source_mode.position.into(),
                    },
                },
            };
            new_windows_modes.push(windows_source_mode);
        }

        // Populate target modes
        for target_mode in self.target_modes.iter() {
            let adapter_id = *device_path_to_adapter_id
                .get(&target_mode.device.adapter.device_instance_path)
                .ok_or_else(|| {
                    anyhow!(
                        "Adapter ID not found for device path: {:?}",
                        target_mode.device.adapter.device_instance_path
                    )
                })?;

            // First find existing target mode that matches desired target mode
            let existing_target_mode_id =
                windows_display_config.get_matching_target_mode_id(adapter_id, target_mode)?;

            let windows_target_mode = DISPLAYCONFIG_MODE_INFO {
                id: existing_target_mode_id,
                adapterId: adapter_id.into(),
                infoType: DISPLAYCONFIG_MODE_INFO_TYPE_TARGET,
                Anonymous: DISPLAYCONFIG_MODE_INFO_0 {
                    targetMode: DISPLAYCONFIG_TARGET_MODE {
                        targetVideoSignalInfo: DISPLAYCONFIG_VIDEO_SIGNAL_INFO {
                            pixelRate: target_mode.pixel_rate.into(),
                            hSyncFreq: target_mode.h_sync_freq.into(),
                            vSyncFreq: target_mode.v_sync_freq.into(),
                            activeSize: target_mode.active_size.into(),
                            totalSize: target_mode.total_size.into(),
                            Anonymous: DISPLAYCONFIG_VIDEO_SIGNAL_INFO_0 {
                                videoStandard: target_mode.video_standard.discriminant() as u32,
                            },
                            scanLineOrdering: target_mode.scanline_ordering.into(),
                        },
                    },
                },
            };
            new_windows_modes.push(windows_target_mode);
        }

        // Populate paths
        for path in self.paths.iter() {
            // Get source and target modes
            let source_windows_mode = new_windows_modes[path.source.source_mode_index];
            assert!(source_windows_mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE);
            let target_windows_mode =
                new_windows_modes[path.target.target_mode_index + self.source_modes.len()];
            assert!(target_windows_mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_TARGET);

            // Get source and target mode indices
            let source_mode_index = path.source.source_mode_index as u32;
            let target_mode_index =
                (path.target.target_mode_index + self.source_modes.len()) as u32;

            let windows_path = DISPLAYCONFIG_PATH_INFO {
                sourceInfo: DISPLAYCONFIG_PATH_SOURCE_INFO {
                    adapterId: source_windows_mode.adapterId,
                    id: source_windows_mode.id,
                    Anonymous: DISPLAYCONFIG_PATH_SOURCE_INFO_0 {
                        modeInfoIdx: source_mode_index,
                    },
                    statusFlags: DISPLAYCONFIG_SOURCE_IN_USE,
                },
                targetInfo: DISPLAYCONFIG_PATH_TARGET_INFO {
                    adapterId: target_windows_mode.adapterId,
                    id: target_windows_mode.id,
                    Anonymous: DISPLAYCONFIG_PATH_TARGET_INFO_0 {
                        modeInfoIdx: target_mode_index,
                    },
                    outputTechnology: path.target.output_technology.into(),
                    rotation: path.target.rotation.into(),
                    scaling: path.target.scaling.into(),
                    refreshRate: path.target.refresh_rate.into(),
                    scanLineOrdering: path.target.scanline_ordering.into(),
                    targetAvailable: true.into(),
                    statusFlags: DISPLAYCONFIG_TARGET_IN_USE,
                },
                flags: DISPLAYCONFIG_PATH_ACTIVE,
            };
            new_windows_paths.push(windows_path);
        }

        Ok(WindowsDisplayConfig::from_paths_and_modes(
            new_windows_paths,
            new_windows_modes,
        )?)
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
pub struct DisplaySourceDevice {
    pub id: u32,
    pub adapter: Adapter,
    #[serde(with = "crate::serde_override::os_string")]
    pub gdi_device_name: OsString,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplaySourceMode {
    pub device: DisplaySourceDevice,
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
