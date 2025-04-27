use std::{
    collections::HashSet,
    ffi::OsString,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use rocket::fairing::AdHoc;
use tracing::info;
use windows::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST;
use windows_service::{
    define_windows_service,
    service::{
        Service, ServiceAccess, ServiceAction, ServiceActionType, ServiceControl,
        ServiceControlAccept, ServiceErrorControl, ServiceExitCode, ServiceFailureActions,
        ServiceFailureResetPeriod, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

pub const SERVICE_NAME: &str = "hagias";
pub const SERVICE_DISPLAY_NAME: &str = "Hagias Monitor Service";
pub const SERVICE_DESCRIPTION: &str =
    "Runs a web server that can be used to change the monitor layout of the system.";

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_TIMEOUT: Option<Duration> = Some(Duration::from_secs(60));

static SERVICE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static SERVICE_RETURN: std::sync::Mutex<Option<anyhow::Error>> = std::sync::Mutex::new(None);
static SERVICE_ROCKET_SHUTDOWN: std::sync::Mutex<Option<rocket::Shutdown>> =
    std::sync::Mutex::new(None);

define_windows_service!(ffi_service_main, service_main);

/// The entry point where execution will start on a background thread after a call to
/// `service_dispatcher::start` from `main`.
fn service_main(args: Vec<OsString>) {
    let result = crate::get_tokio_handle().block_on(async { service_main_async(args).await });
    if let Err(e) = result {
        SERVICE_RETURN
            .lock()
            .expect("failed to lock service return")
            .replace(e);
    }
}

async fn service_main_async(_args: Vec<OsString>) -> Result<()> {
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

fn get_service_manager(manager_access: ServiceManagerAccess) -> Result<ServiceManager> {
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)
        .with_context(|| {
            format!(
                "failed to create service manager with access {:?}",
                manager_access
            )
        })?;
    Ok(service_manager)
}

fn get_service(service_manager: &ServiceManager, service_access: ServiceAccess) -> Result<Service> {
    service_manager
        .open_service(SERVICE_NAME, service_access)
        .with_context(|| {
            format!(
                "failed to get service '{}' with access {:?}",
                SERVICE_NAME, service_access
            )
        })
}

fn get_service_opt(
    service_manager: &ServiceManager,
    service_access: ServiceAccess,
) -> Result<Option<Service>> {
    match service_manager.open_service(SERVICE_NAME, service_access) {
        Ok(service) => Ok(Some(service)),
        Err(windows_service::Error::Winapi(e))
            if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST.0 as i32) =>
        {
            Ok(None)
        }
        Err(e) => Err(e).with_context(|| {
            format!(
                "failed to get service '{}' with access {:?}",
                SERVICE_NAME, service_access
            )
        }),
    }
}

/// Waits until the service reaches the target state.
///
/// If the target state is `None`, the service will be waited until it is deleted.
async fn wait_until_service_state_is(
    service: &Service,
    allowed_states: HashSet<ServiceState>,
    target_state: Option<ServiceState>,
    poll_interval: Duration,
    timeout: Option<Duration>,
) -> Result<()> {
    let start = Instant::now();
    loop {
        let status_result = service.query_status();
        if target_state.is_none() {
            // If the target state is `None`, the service will be waited until it is deleted (when the service does not exist)
            if let Err(windows_service::Error::Winapi(e)) = &status_result {
                if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST.0 as i32) {
                    return Ok(());
                }
            }
        }
        let status = status_result
            .with_context(|| format!("failed to query service '{}' status", SERVICE_NAME))?;
        if target_state.is_some() && status.current_state == target_state.unwrap() {
            // If the target state is `Some`, the service will be waited until it reaches the target state
            return Ok(());
        } else if !allowed_states.contains(&status.current_state) {
            // If the service is not in the allowed states, we return an error
            return Err(anyhow::anyhow!(
                "service {} failed to reach state {:?} (unexpected current state: {:?})",
                SERVICE_NAME,
                target_state,
                status.current_state
            ));
        }
        if let Some(timeout) = timeout {
            if start.elapsed() > timeout {
                // If the timeout is reached, we return an error
                return Err(anyhow::anyhow!(
                    "service {} timed out while waiting to reach state {:?} (current state: {:?})",
                    SERVICE_NAME,
                    target_state,
                    status.current_state
                ));
            }
        }
        // Sleep for the poll interval
        tokio::time::sleep(poll_interval).await;
    }
}

pub async fn unregister_if_exists() -> Result<()> {
    let service_manager = get_service_manager(ServiceManagerAccess::CONNECT)?;
    if let Some(service) = get_service_opt(
        &service_manager,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
    )? {
        unregister_common(service_manager, service).await?;
    } else {
        info!("Service '{}' does not exist", SERVICE_NAME);
    }
    Ok(())
}

pub async fn unregister() -> Result<()> {
    let service_manager = get_service_manager(ServiceManagerAccess::CONNECT)?;

    let service = get_service(
        &service_manager,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
    )?;
    unregister_common(service_manager, service).await?;
    Ok(())
}

async fn unregister_common(
    service_manager: ServiceManager,
    service: Service,
) -> Result<(), anyhow::Error> {
    info!("Deleting service '{}'", SERVICE_NAME);
    service
        .delete()
        .with_context(|| format!("failed to delete service '{}'", SERVICE_NAME))?;
    info!("Checking if service '{}' is stopped", SERVICE_NAME);
    if query_status(&service)?.current_state != ServiceState::Stopped {
        info!("Stopping service '{}'", SERVICE_NAME);
        service
            .stop()
            .with_context(|| format!("failed to stop service '{}'", SERVICE_NAME))?;
    } else {
        info!("Service '{}' is already stopped", SERVICE_NAME);
    }
    drop(service);
    info!("Waiting for service '{}' to be deleted", SERVICE_NAME);
    if let Some(service) = get_service_opt(&service_manager, ServiceAccess::QUERY_STATUS)? {
        wait_until_service_state_is(
            &service,
            HashSet::from([ServiceState::StopPending, ServiceState::Stopped]),
            None,
            DEFAULT_POLL_INTERVAL,
            DEFAULT_TIMEOUT,
        )
        .await?;
    }
    info!("Service '{}' has been deleted", SERVICE_NAME);
    Ok(())
}

pub async fn register(start: bool) -> Result<()> {
    let service_manager =
        get_service_manager(ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)?;

    let service_binary_path =
        std::env::current_exe().context("failed to get current executable path")?;

    info!(
        "Registering service {}: {}",
        SERVICE_NAME,
        service_binary_path.display()
    );
    let service_info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_binary_path,
        launch_arguments: vec!["service".into(), "run".into()],
        dependencies: vec![],
        account_name: None, // run as System
        account_password: None,
    };
    let service = service_manager
        .create_service(
            &service_info,
            ServiceAccess::CHANGE_CONFIG | ServiceAccess::QUERY_STATUS | ServiceAccess::START,
        )
        .with_context(|| format!("failed to create service '{}'", SERVICE_NAME))?;
    info!("Service '{}' registered", SERVICE_NAME);

    info!("Setting description for service '{}'", SERVICE_NAME);
    service
        .set_description(SERVICE_DESCRIPTION)
        .with_context(|| format!("failed to set description for service '{}'", SERVICE_NAME))?;
    info!("Set description for service '{}'", SERVICE_NAME);

    info!("Setting failure actions for service '{}'", SERVICE_NAME);
    service
        .update_failure_actions(ServiceFailureActions {
            reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(60 * 60)),
            reboot_msg: None,
            command: None,
            actions: Some(vec![ServiceAction {
                action_type: ServiceActionType::Restart,
                delay: Duration::from_secs(60),
            }]),
        })
        .with_context(|| {
            format!(
                "failed to set failure actions for service '{}'",
                SERVICE_NAME
            )
        })?;
    info!("Set failure actions for service '{}'", SERVICE_NAME);

    if start {
        start_common(&service).await
    } else {
        info!("Service '{}' registered but not started", SERVICE_NAME);
        Ok(())
    }
}

pub async fn start() -> Result<()> {
    let service_manager = get_service_manager(ServiceManagerAccess::CONNECT)?;
    let service = get_service(
        &service_manager,
        ServiceAccess::QUERY_STATUS | ServiceAccess::START,
    )?;
    start_common(&service).await
}

async fn start_common(service: &Service) -> Result<()> {
    let current_state = query_status(&service)?.current_state;
    if current_state == ServiceState::Running {
        info!("Service '{}' is already running", SERVICE_NAME);
        return Ok(());
    } else if current_state == ServiceState::StartPending {
        info!("Service '{}' is already starting", SERVICE_NAME);
    } else {
        info!("Starting service '{}'", SERVICE_NAME);
        service
            .start::<&str>(&[])
            .with_context(|| format!("failed to start service '{}'", SERVICE_NAME))?;
    }
    info!("Waiting for service '{}' to start", SERVICE_NAME);
    wait_until_service_state_is(
        &service,
        HashSet::from([ServiceState::StartPending]),
        Some(ServiceState::Running),
        DEFAULT_POLL_INTERVAL,
        DEFAULT_TIMEOUT,
    )
    .await?;
    info!("Service '{}' started", SERVICE_NAME);
    Ok(())
}

pub async fn stop() -> Result<()> {
    let service_manager = get_service_manager(ServiceManagerAccess::CONNECT)?;
    let service = get_service(
        &service_manager,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP,
    )?;
    stop_common(&service).await
}

async fn stop_common(service: &Service) -> Result<()> {
    let current_state = query_status(&service)?.current_state;
    if current_state == ServiceState::Stopped {
        info!("Service '{}' is already stopped", SERVICE_NAME);
        return Ok(());
    } else if current_state == ServiceState::StopPending {
        info!("Service '{}' is already stopping", SERVICE_NAME);
    } else {
        info!("Stopping service '{}'", SERVICE_NAME);
        service
            .stop()
            .with_context(|| format!("failed to stop service '{}'", SERVICE_NAME))?;
    }
    info!("Waiting for service '{}' to stop", SERVICE_NAME);
    wait_until_service_state_is(
        &service,
        HashSet::from([ServiceState::Running, ServiceState::StopPending]),
        Some(ServiceState::Stopped),
        DEFAULT_POLL_INTERVAL,
        DEFAULT_TIMEOUT,
    )
    .await?;
    info!("Service '{}' stopped", SERVICE_NAME);
    Ok(())
}

pub async fn restart() -> Result<()> {
    let service_manager = get_service_manager(ServiceManagerAccess::CONNECT)?;
    let service = get_service(
        &service_manager,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::START,
    )?;
    let current_status = query_status(&service)?;
    match current_status.current_state {
        ServiceState::StartPending | ServiceState::Running => {
            stop_common(&service).await?;
            start_common(&service).await?;
        }
        ServiceState::StopPending => {
            info!("Waiting for service '{}' to stop", SERVICE_NAME);
            wait_until_service_state_is(
                &service,
                HashSet::from([ServiceState::StopPending]),
                Some(ServiceState::Stopped),
                DEFAULT_POLL_INTERVAL,
                DEFAULT_TIMEOUT,
            )
            .await?;
            info!("Service '{}' stopped", SERVICE_NAME);
            start_common(&service).await?;
        }
        ServiceState::Stopped => {
            info!("Service '{}' is already stopped", SERVICE_NAME);
            start_common(&service).await?;
        }
        _ => {
            return Err(anyhow::anyhow!(
                "service {} failed to restart (unexpected current state: {:?})",
                SERVICE_NAME,
                current_status.current_state
            ));
        }
    }
    info!("Service '{}' restarted", SERVICE_NAME);
    Ok(())
}

pub async fn status() -> Result<Option<ServiceStatus>> {
    let service_manager = get_service_manager(ServiceManagerAccess::CONNECT)?;
    let service = get_service_opt(&service_manager, ServiceAccess::QUERY_STATUS)?;
    if let Some(service) = service {
        query_status_opt(&service)
    } else {
        Ok(None)
    }
}

fn query_status(service: &Service) -> Result<ServiceStatus> {
    service
        .query_status()
        .with_context(|| format!("failed to query service '{}' status", SERVICE_NAME))
}

fn query_status_opt(service: &Service) -> Result<Option<ServiceStatus>> {
    let result = service.query_status();
    if let Err(windows_service::Error::Winapi(e)) = &result {
        if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST.0 as i32) {
            return Ok(None);
        }
    }
    result
        .with_context(|| format!("failed to query service '{}' status", SERVICE_NAME))
        .map(|s| Some(s))
}
