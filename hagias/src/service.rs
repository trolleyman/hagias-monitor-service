use std::{
    ffi::OsString,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use rocket::fairing::AdHoc;
use tracing::{info, info_span};
use windows::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST;
use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

pub const SERVICE_NAME: &str = "hagias";

pub const SERVICE_DISPLAY_NAME: &str = "Hagias Monitor Service";

pub const SERVICE_DESCRIPTION: &str =
    "Runs a web server that can be used to change the monitor layout of the system.";

static SERVICE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

static SERVICE_RETURN: std::sync::Mutex<Option<anyhow::Error>> = std::sync::Mutex::new(None);

static SERVICE_ROCKET_SHUTDOWN: std::sync::Mutex<Option<rocket::Shutdown>> =
    std::sync::Mutex::new(None);

define_windows_service!(ffi_service_main, service_main);

/// The entry point where execution will start on a background thread after a call to
/// `service_dispatcher::start` from `main`.
fn service_main(args: Vec<OsString>) {
    let _ = info_span!("service_main").entered();
    let result = crate::get_tokio_handle().block_on(async { service_main_async(args).await });
    if let Err(e) = result {
        SERVICE_RETURN
            .lock()
            .expect("failed to lock service return")
            .replace(e);
    }
}

async fn service_main_async(_args: Vec<OsString>) -> Result<()> {
    let _ = info_span!("service_main_async").entered();
    let (rocket, status_handle) = {
        info!("Setting up service {}", SERVICE_NAME);
        // Lock the rocket shutdown mutex so we don't access it via. the event handler while it's being set
        let mut shutdown_lock = SERVICE_ROCKET_SHUTDOWN
            .lock()
            .expect("failed to lock rocket shutdown");
        shutdown_lock.take(); // Reset the shutdown mutex

        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop => {
                    // Handle stop event and return control back to the system.
                    let mut lock = SERVICE_ROCKET_SHUTDOWN.try_lock();
                    if let Ok(ref mut mutex) = lock {
                        // Notify the rocket shutdown immediately if we can lock the mutex
                        if let Some(ref shutdown) = **mutex {
                            shutdown.clone().notify();
                        }
                    } else {
                        // Spawn a thread if we can't lock the mutex
                        std::thread::spawn(move || {
                            let lock = SERVICE_ROCKET_SHUTDOWN
                                .lock()
                                .expect("failed to lock rocket shutdown");
                            if let Some(ref shutdown) = *lock {
                                shutdown.clone().notify();
                            }
                        });
                    }
                    ServiceControlHandlerResult::NoError
                }
                // All services must accept Interrogate even if it's a no-op.
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        // Register service
        info!("Registering service {}", SERVICE_NAME);
        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StartPending,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::from_secs(60),
            process_id: None,
        })?;

        info!("Getting configs");
        let (figment, config) = crate::config::get()?;
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StartPending,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 1,
            wait_hint: Duration::from_secs(60),
            process_id: None,
        })?;

        info!("Building rocket");
        let status_handle_clone = status_handle.clone();
        let rocket = crate::get_rocket_build(figment, config).attach(AdHoc::on_liftoff(
            "Liftoff Printer",
            move |r| {
                Box::pin(async move {
                    if let Err(e) = status_handle_clone.set_service_status(ServiceStatus {
                        service_type: ServiceType::OWN_PROCESS,
                        current_state: ServiceState::Running,
                        controls_accepted: ServiceControlAccept::STOP,
                        exit_code: ServiceExitCode::Win32(0),
                        checkpoint: 0,
                        wait_hint: Duration::default(),
                        process_id: None,
                    }) {
                        eprintln!("failed to set service status: {}", e);
                        r.shutdown().notify();
                    }
                })
            },
        ));
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StartPending,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 2,
            wait_hint: Duration::from_secs(60),
            process_id: None,
        })?;

        info!("Igniting rocket");
        let rocket = match crate::ignite_rocket(rocket).await {
            Ok(rocket) => rocket,
            Err(e) => {
                let _ = status_handle.set_service_status(ServiceStatus {
                    service_type: ServiceType::OWN_PROCESS,
                    current_state: ServiceState::Stopped,
                    controls_accepted: ServiceControlAccept::STOP,
                    exit_code: ServiceExitCode::Win32(1),
                    checkpoint: 0,
                    wait_hint: Duration::default(),
                    process_id: None,
                });
                return Err(e);
            }
        };

        // Update status
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StartPending,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 3,
            wait_hint: Duration::from_secs(60),
            process_id: None,
        })?;

        // Replace the rocket shutdown mutex with the new shutdown notifier
        shutdown_lock.replace(rocket.shutdown());
        (rocket, status_handle)
    };

    // Launch rocket (starts rocket)
    info!("Launching rocket");
    let result = crate::launch_rocket(rocket).await;

    let status_handle_result = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(if result.is_ok() { 0 } else { 1 }),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    });
    result?;
    status_handle_result?;
    Ok(())
}

pub fn run() -> Result<()> {
    let _ = info_span!("service::run").entered();
    // Ensure that we have exclusive access to the service
    let _service_lock = SERVICE_LOCK.lock().expect("failed to lock service lock");

    // Clear any previous error
    SERVICE_RETURN
        .lock()
        .expect("failed to lock service return")
        .take();

    // Start the service
    info!("Starting service {}", SERVICE_NAME);
    service_dispatcher::start(SERVICE_NAME, ffi_service_main).context("service error")?;
    info!("Service {} finished", SERVICE_NAME);

    // Return any error that occurred
    if let Some(error) = SERVICE_RETURN
        .lock()
        .expect("failed to lock service return")
        .take()
    {
        return Err(anyhow::anyhow!(error).context("service error"));
    }

    // `_service_lock` is released when the function returns
    Ok(())
}

pub async fn unregister() -> Result<()> {
    let _ = info_span!("service::unregister").entered();
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)
        .context("failed to create service manager")?;

    info!("Checking if service {} exists", SERVICE_NAME);
    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE;
    let service = service_manager
        .open_service(SERVICE_NAME, service_access)
        .with_context(|| {
            format!(
                "failed to open service '{}' for querying/stopping/deleting",
                SERVICE_NAME
            )
        })?;

    info!("Deleting service {}", SERVICE_NAME);
    // The service will be marked for deletion as long as this function call succeeds.
    // However, it will not be deleted from the database until it is stopped and all open handles to it are closed.
    service
        .delete()
        .with_context(|| format!("failed to delete service '{}'", SERVICE_NAME))?;
    // Our handle to it is not closed yet. So we can still query it.
    if service
        .query_status()
        .with_context(|| format!("failed to query service '{}' status", SERVICE_NAME))?
        .current_state
        != ServiceState::Stopped
    {
        // If the service cannot be stopped, it will be deleted when the system restarts.
        info!("Stopping service {}", SERVICE_NAME);
        service
            .stop()
            .with_context(|| format!("failed to stop service '{}'", SERVICE_NAME))?;
    }
    // Explicitly close our open handle to the service. This is automatically called when `service` goes out of scope.
    drop(service);

    // Win32 API does not give us a way to wait for service deletion.
    // To check if the service is deleted from the database, we have to poll it ourselves.
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    while start.elapsed() < timeout {
        if let Err(windows_service::Error::Winapi(e)) =
            service_manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)
        {
            if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST.0 as i32) {
                break;
            } else {
                return Err(anyhow::anyhow!(e)
                    .context(format!("failed to query service '{}' status", SERVICE_NAME)));
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    info!("Service {} has been deleted", SERVICE_NAME);
    Ok(())
}

pub async fn unregister_if_exists() -> Result<()> {
    let _ = info_span!("service::unregister_if_exists").entered();
    {
        let manager_access = ServiceManagerAccess::CONNECT;
        let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)
            .context("failed to create service manager")?;

        info!("Checking if service {} exists", SERVICE_NAME);
        if let Err(windows_service::Error::Winapi(e)) =
            service_manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)
        {
            if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST.0 as i32) {
                info!("Service {} does not exist", SERVICE_NAME);
                return Ok(());
            } else {
                return Err(anyhow::anyhow!(e)
                    .context(format!("failed to query service '{}' status", SERVICE_NAME)));
            }
        }
    }
    unregister().await
}

pub fn register() -> Result<()> {
    let _ = info_span!("service::register").entered();
    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)
        .context("failed to create service manager")?;

    // This example installs the service defined in `examples/ping_service.rs`.
    // In the real world code you would set the executable path to point to your own binary
    // that implements windows service.
    let service_binary_path =
        std::env::current_exe().context("failed to get current executable path")?;

    info!("Registering service {}", SERVICE_NAME);
    let service_info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::OnDemand,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_binary_path,
        launch_arguments: vec!["service".into(), "run".into()],
        dependencies: vec![],
        account_name: None, // run as System
        account_password: None,
    };
    let service = service_manager
        .create_service(&service_info, ServiceAccess::CHANGE_CONFIG)
        .with_context(|| format!("failed to create service '{}'", SERVICE_NAME))?;
    service
        .set_description(SERVICE_DESCRIPTION)
        .with_context(|| format!("failed to set description for service '{}'", SERVICE_NAME))?;
    Ok(())
}
