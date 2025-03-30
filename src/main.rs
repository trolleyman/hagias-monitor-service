use anyhow::{Context, Result, bail};

use clap::Parser;
use windows::{
    Win32::{
        Devices::Display::{
            DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_MODE_INFO_TYPE_DESKTOP_IMAGE,
            DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE, DISPLAYCONFIG_MODE_INFO_TYPE_TARGET,
            DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_TOPOLOGY_ID, GetDisplayConfigBufferSizes,
            QDC_ALL_PATHS, QDC_DATABASE_CURRENT, QDC_ONLY_ACTIVE_PATHS, QUERY_DISPLAY_CONFIG_FLAGS,
            QueryDisplayConfig, SDC_APPLY, SDC_USE_SUPPLIED_DISPLAY_CONFIG,
            SET_DISPLAY_CONFIG_FLAGS, SetDisplayConfig,
        },
        Foundation::{
            ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, GetLastError, HLOCAL, LocalFree, WIN32_ERROR,
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

#[derive(Debug, Clone, clap::Parser)]
#[command(author, version, about)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    /// Edit configuration
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum ConfigCommand {
    // Store the current monitor configuration as the config named `name`
    Store { id: String, name: String },
}

#[rocket::main]
pub async fn main() -> Result<()> {
    std::process::exit(run().await?)
}

pub async fn run() -> Result<i32> {
    let args = Args::parse();

    if let Some(command) = args.command {
        if let Some(code) = run_command(command).await? {
            return Ok(code);
        }
    }

    rocket::build().launch().await.context("Rocket error")?;
    Ok(0)
}

pub async fn run_command(command: Command) -> Result<Option<i32>> {
    match command {
        Command::Config(config_command) => match config_command {
            ConfigCommand::Store { id, name } => {
                store_monitor_config(&id, &name).await?;
                Ok(Some(0))
            }
        },
    }
}

pub fn windows_error_to_string(error: WIN32_ERROR) -> Result<String> {
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
            let last_error = GetLastError();
            bail!(
                "windows_error_to_string({:?}) error: {:?}",
                error,
                last_error
            );
        }
        let string_result =
            String::from_utf16(std::slice::from_raw_parts(error_text.0, num_chars as _));
        LocalFree(Some(HLOCAL(error_text.0 as *mut _)));
        match string_result {
            Ok(s) => Ok(s),
            Err(e) => {
                bail!("windows_error_to_string({:?}) UTF-16 error: {:?}", error, e);
            }
        }
    }
}

pub enum WindowsDisplayQueryType {
    All,
    Active,
    Database,
}

impl WindowsDisplayQueryType {
    pub fn to_flags(self) -> QUERY_DISPLAY_CONFIG_FLAGS {
        match self {
            WindowsDisplayQueryType::All => QDC_ALL_PATHS,
            WindowsDisplayQueryType::Active => QDC_ONLY_ACTIVE_PATHS,
            WindowsDisplayQueryType::Database => QDC_DATABASE_CURRENT,
        }
    }
}

pub struct WindowsDisplayConfig {
    paths: Vec<DISPLAYCONFIG_PATH_INFO>,
    modes: Vec<DISPLAYCONFIG_MODE_INFO>,
}

impl WindowsDisplayConfig {
    pub fn get(query: WindowsDisplayQueryType) -> Result<WindowsDisplayConfig> {
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
                        windows_error_to_string(result)?
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
                        "QueryDisplayConfig error: {:?}",
                        windows_error_to_string(result)?
                    );
                }

                paths.set_len(num_paths as usize);
                modes.set_len(num_modes as usize);

                return Ok(WindowsDisplayConfig { paths, modes });
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
                    "SetDisplayConfig error: {:?}",
                    windows_error_to_string(WIN32_ERROR(result as u32))?
                );
            }
        }
        Ok(())
    }
}

pub async fn store_monitor_config(_id: &str, _name: &str) -> Result<()> {
    let mut display_config = WindowsDisplayConfig::get(WindowsDisplayQueryType::Active)?;
    for (i, mode) in display_config.modes.iter().enumerate() {
        println!("Display Mode #{}", i);
        println!("  ID: {:?}", mode.id);
        println!("  Adapter ID: {:?}", mode.adapterId);
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
                        "      HSync Freq: {:?}",
                        target_mode.targetVideoSignalInfo.hSyncFreq
                    );
                    println!(
                        "      VSync Freq: {:?}",
                        target_mode.targetVideoSignalInfo.vSyncFreq
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
                }
                DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE => {
                    let source_mode = mode.Anonymous.sourceMode;
                    println!("  Source Mode:");
                    println!("    Width: {}", source_mode.width);
                    println!("    Height: {}", source_mode.height);
                    println!("    Pixel Format: {:?}", source_mode.pixelFormat);
                    println!("    Position: {:?}", source_mode.position);
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
    for (i, path) in display_config.paths.iter().enumerate() {
        println!("Display Path #{}", i);
        println!("  Source:");
        println!("    ID: {}", path.sourceInfo.id);
        println!("    Adapter ID: {:?}", path.sourceInfo.adapterId);
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
        println!("    Status Flags: {:x}", path.sourceInfo.statusFlags);
        if path.sourceInfo.statusFlags & DISPLAYCONFIG_SOURCE_IN_USE == DISPLAYCONFIG_SOURCE_IN_USE
        {
            println!("      DISPLAYCONFIG_SOURCE_IN_USE");
        }
        println!("  Target:");
        println!("    ID: {}", path.targetInfo.id);
        println!("    Adapter ID: {:?}", path.targetInfo.adapterId);
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
            "    Output Technology: {:?}",
            path.targetInfo.outputTechnology
        );
        println!("    Rotation: {:?}", path.targetInfo.rotation);
        println!("    Scaling: {:?}", path.targetInfo.scaling);
        println!("    Refresh Rate: {:?}", path.targetInfo.refreshRate);
        println!(
            "    Scanline Ordering: {:?}",
            path.targetInfo.scanLineOrdering
        );
        println!(
            "    Target Available: {}",
            path.targetInfo.targetAvailable.as_bool()
        );
        println!("    Status Flags: {:x}", path.targetInfo.statusFlags);
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
        println!("  Flags: {:x}", path.flags);
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

    display_config
        .paths
        .iter_mut()
        .skip(1)
        .for_each(|path| path.flags &= !DISPLAYCONFIG_PATH_ACTIVE);

    display_config.set()?;

    Ok(())
}
