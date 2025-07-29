use anyhow::{Context as _, Result};
use tracing::{error, info};

use crate::config::Config;

#[cfg(feature = "cec")]
pub mod cec;
pub mod layout;
pub mod service;

pub mod rearranger;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    /// Edit layout configuration
    #[command(subcommand)]
    Layout(layout::Command),
    /// Run as a service
    #[command(subcommand)]
    Service(service::Command),
    /// Send a CEC command to a device
    #[cfg(feature = "cec")]
    #[command(subcommand)]
    Cec(cec::Command),
    /// Enumerate displays
    #[cfg(feature = "enum-displays")]
    EnumDisplays,
}
impl Command {
    pub async fn run(&self, config: &Config) -> Result<Option<i32>> {
        let command_debug = format!("{:?}", self);
        info!("Running command: {}", command_debug);
        let result = match self {
            Command::Layout(layout_command) => layout_command.run(config).await,
            Command::Service(service_command) => service_command.run(config).await,
            #[cfg(feature = "cec")]
            Command::Cec(cec_command) => cec_command.run(config).await,
            #[cfg(feature = "enum-displays")]
            Command::EnumDisplays => enum_displays::run(config).await,
        };
        if let Err(ref e) = result {
            error!("Command failed: {}", e);
        }
        result.with_context(|| format!("Command failed: {}", command_debug))
    }
}

#[cfg(feature = "enum-displays")]
mod enum_displays {
    use anyhow::Result;
    use windows::{
        core::BOOL, Win32::{
            Devices::Display::{
                DestroyPhysicalMonitors, GetNumberOfPhysicalMonitorsFromHMONITOR, PHYSICAL_MONITOR,
            },
            Foundation::{ERROR_SUCCESS, LPARAM, RECT},
            Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR},
        }
    };

    use crate::config::Config;

    struct PhysicalMonitors(Vec<PHYSICAL_MONITOR>);

    impl Drop for PhysicalMonitors {
        fn drop(&mut self) {
            unsafe {
                DestroyPhysicalMonitors(&self.0[..]);
                self.0.set_len(0);
            }
        }
    }

    async fn run(_config: &Config) -> Result<Option<i32>> {
        let physical_monitors = get_physical_monitors()?;
        println!("Physical monitors detected {}", physical_monitors.len());
        for physical_monitor in physical_monitors {
            println!("Physical monitor: {:?}", physical_monitor);
        }
        Ok(Some(0))
    }

    /// Get all physical monitors on the system.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it uses the `EnumDisplayMonitors` function, which is a system call.
    unsafe fn get_physical_monitors() -> Result<PhysicalMonitors> {
        const _: () = assert!(
            std::mem::size_of::<LPARAM>() == std::mem::size_of::<*mut PhysicalMonitors>(),
            "LPARAM and *mut Vec<PHYSICAL_MONITOR> must be the same size"
        );

        unsafe extern "system" fn callback(
            monitor: HMONITOR,
            _hdc: HDC,
            _rect: RECT,
            lparam: LPARAM,
        ) -> BOOL {
            unsafe {
                let new_physical_monitors = get_physical_monitors_from_hmonitor(monitor)?;
                let physical_monitors =
                    &mut *std::mem::transmute::<LPARAM, *mut PhysicalMonitors>(lparam);
                physical_monitors.push(physical_monitor);
                BOOL::from(true)
            }
        }

        let mut physical_monitors = PhysicalMonitors(Vec::<PHYSICAL_MONITOR>::new());
        {
            unsafe {
                EnumDisplayMonitors(None, None, Some(callback), 0);
            }
        }
        Ok(physical_monitors)
    }

    unsafe fn get_physical_monitors_from_hmonitor(
        hmonitor: HMONITOR,
    ) -> Result<Vec<PHYSICAL_MONITOR>> {
        let mut num_physical_monitors = 0;
        let result = GetNumberOfPhysicalMonitorsFromHMONITOR(hmonitor, &mut num_physical_monitors);
        if result != ERROR_SUCCESS {
            bail!(
                "GetNumberOfPhysicalMonitorsFromHMONITOR error: {}",
                windows_error_to_string(result)
            );
        }
    }
}
