#[cfg(target_os = "windows")]
mod platform {
    use std::ptr;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use serde::Serialize;
    use tauri::{AppHandle, Emitter};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::Memory::{
        FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, OpenFileMappingW,
        UnmapViewOfFile,
    };
    use crate::features::career::logbook::{self, TelemetrySample};
    use crate::state::CareerRuntime;

    const SHARED_MEMORY_NAME: &str = "Local\\SCSTelemetry";
    const SHARED_MEMORY_SIZE: usize = 32 * 1024;
    const POLL_INTERVAL: Duration = Duration::from_millis(150);
    const RECONNECT_INTERVAL: Duration = Duration::from_millis(1000);
    const ZONE1_OFFSET: usize = 0;
    const ZONE3_OFFSET: usize = 500;
    const ZONE4_OFFSET: usize = 700;
    const ZONE5_OFFSET: usize = 1500;
    const NOT_AVAILABLE_MESSAGE: &str =
        "Telemetry not available. Is ETS2 running with the plugin?";

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Zone1 {
        sdk_active: u8,
        _sdk_padding: [u8; 3],
        paused: u8,
        _paused_padding: [u8; 3],
        timestamp: u64,
        simulation_timestamp: u64,
        render_timestamp: u64,
        _multiplayer_time_offset: i64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Zone3 {
        _next_rest_stop: u32,
        selected_gear: i32,
        dashboard_gear: i32,
        _slot_gears: [i32; 32],
        _earned_xp: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Zone4 {
        _prefix: [f32; 13],
        _wheel_radius: [f32; 16],
        _gear_ratios_forward: [f32; 24],
        _gear_ratios_reverse: [f32; 8],
        _unit_mass: f32,
        speed_mps: f32,
        rpm: f32,
        _raw_input: [f32; 8],
        _cruise_control_speed_mps: f32,
        _air_pressure: f32,
        _brake_temperature: f32,
        fuel_liters: f32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Zone5 {
        _wheel_steerable: [u8; 16],
        _wheel_simulated: [u8; 16],
        _wheel_powered: [u8; 16],
        _wheel_liftable: [u8; 16],
        _cargo_loaded: u8,
        _special_job: u8,
        _parking_brake: u8,
        _motor_brake: u8,
        _warning_air_pressure: u8,
        _warning_air_pressure_emergency: u8,
        _warning_fuel: u8,
        _warning_ad_blue: u8,
        _warning_oil_pressure: u8,
        _warning_water_temperature: u8,
        _warning_battery_voltage: u8,
        _electric_enabled: u8,
        engine_enabled: u8,
    }

    #[derive(Debug, Clone, Serialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct FrontendTelemetryPayload {
        speed: f32,
        rpm: f32,
        gear: String,
        fuel: f32,
        fuel_capacity: f32,
        engine_on: bool,
        timestamp: u64,
        paused: bool,
        plugin_installed: bool,
        sdk_connected: bool,
    }

    impl TelemetrySample {
        fn format_line(self) -> String {
            format!(
                "Speed: {} km/h | RPM: {} | Gear: {} | Fuel: {}L | Engine: {}",
                self.speed_kph.round() as i32,
                self.rpm.round() as i32,
                format_gear(self.gear),
                self.fuel_liters.round() as i32,
                if self.engine_enabled { "ON" } else { "OFF" }
            )
        }

        fn into_frontend_payload(self, plugin_installed: bool, sdk_connected: bool) -> FrontendTelemetryPayload {
            FrontendTelemetryPayload {
                speed: self.speed_kph,
                rpm: self.rpm,
                gear: format_gear(self.gear),
                fuel: self.fuel_liters,
                fuel_capacity: self.fuel_capacity_liters,
                engine_on: self.engine_enabled,
                timestamp: self.timestamp,
                paused: self.paused,
                plugin_installed,
                sdk_connected,
            }
        }
    }

    struct SharedTelemetryMap {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
    }

    impl SharedTelemetryMap {
        fn connect() -> Result<Self, String> {
            let segment_name = wide_null(SHARED_MEMORY_NAME);
            let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, segment_name.as_ptr()) };
            if handle.is_null() {
                return Err(NOT_AVAILABLE_MESSAGE.to_string());
            }

            let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, SHARED_MEMORY_SIZE) };
            if view.Value.is_null() {
                unsafe {
                    CloseHandle(handle);
                }
                return Err(NOT_AVAILABLE_MESSAGE.to_string());
            }

            Ok(Self { handle, view })
        }

        fn read_snapshot(&self) -> Result<Option<TelemetrySample>, String> {
            let zone1_before = unsafe { self.read_zone::<Zone1>(ZONE1_OFFSET) };
            if zone1_before.sdk_active == 0 {
                return Ok(None);
            }

            let zone3 = unsafe { self.read_zone::<Zone3>(ZONE3_OFFSET) };
            let zone4 = unsafe { self.read_zone::<Zone4>(ZONE4_OFFSET) };
            let zone5 = unsafe { self.read_zone::<Zone5>(ZONE5_OFFSET) };
            let zone1_after = unsafe { self.read_zone::<Zone1>(ZONE1_OFFSET) };

            if zone1_after.sdk_active == 0 {
                return Ok(None);
            }

            let token_before = change_token(zone1_before);
            let token_after = change_token(zone1_after);
            if token_before == 0 || token_before != token_after {
                return Ok(None);
            }

            let gear = if zone3.dashboard_gear != 0 {
                zone3.dashboard_gear
            } else {
                zone3.selected_gear
            };

            Ok(Some(TelemetrySample {
                timestamp: token_after,
                speed_kph: zone4.speed_mps * 3.6,
                rpm: zone4.rpm,
                gear,
                fuel_liters: zone4.fuel_liters,
                fuel_capacity_liters: zone4._prefix[1],
                engine_enabled: zone5.engine_enabled != 0,
                paused: zone1_after.paused != 0,
            }))
        }

        unsafe fn read_zone<T: Copy>(&self, offset: usize) -> T {
            unsafe {
                ptr::read_volatile((self.view.Value as *const u8).add(offset) as *const T)
            }
        }
    }

    impl Drop for SharedTelemetryMap {
        fn drop(&mut self) {
            unsafe {
                if !self.view.Value.is_null() {
                    UnmapViewOfFile(self.view);
                }
                if !self.handle.is_null() {
                    CloseHandle(self.handle);
                }
            }
        }
    }

    pub fn start_terminal_telemetry_loop() {
        if let Err(error) = thread::Builder::new()
            .name("scs-sdk-telemetry-terminal".to_string())
            .spawn(telemetry_loop)
        {
            eprintln!("Failed to start telemetry thread: {error}");
        }
    }

    pub fn start_frontend_telemetry_bridge(app: AppHandle, runtime: Arc<CareerRuntime>) {
        if let Err(error) = thread::Builder::new()
            .name("scs-sdk-telemetry-frontend".to_string())
            .spawn(move || frontend_telemetry_loop(app, runtime))
        {
            eprintln!("Failed to start telemetry frontend thread: {error}");
        }
    }

    fn telemetry_loop() {
        let mut shared_map: Option<SharedTelemetryMap> = None;
        let mut last_timestamp: Option<u64> = None;
        let mut unavailable_logged = false;

        loop {
            if shared_map.is_none() {
                match SharedTelemetryMap::connect() {
                    Ok(map) => {
                        shared_map = Some(map);
                        last_timestamp = None;
                        unavailable_logged = false;
                    }
                    Err(message) => {
                        if !unavailable_logged {
                            println!("{message}");
                            unavailable_logged = true;
                        }
                        thread::sleep(RECONNECT_INTERVAL);
                        continue;
                    }
                }
            }

            match shared_map.as_ref().unwrap().read_snapshot() {
                Ok(Some(snapshot)) => {
                    unavailable_logged = false;
                    if last_timestamp != Some(snapshot.timestamp) {
                        last_timestamp = Some(snapshot.timestamp);
                        println!("{}", snapshot.format_line());
                    }
                }
                Ok(None) => {
                    if !unavailable_logged {
                        println!("{NOT_AVAILABLE_MESSAGE}");
                        unavailable_logged = true;
                    }
                    last_timestamp = None;
                }
                Err(message) => {
                    shared_map = None;
                    last_timestamp = None;
                    if !unavailable_logged {
                        println!("{message}");
                        unavailable_logged = true;
                    }
                }
            }

            thread::sleep(POLL_INTERVAL);
        }
    }

    fn frontend_telemetry_loop(app: AppHandle, runtime: Arc<CareerRuntime>) {
        let mut shared_map: Option<SharedTelemetryMap> = None;
        let mut last_timestamp: Option<u64> = None;
        let mut last_payload: Option<FrontendTelemetryPayload> = None;

        while !runtime.stop_all.load(Ordering::Relaxed) {
            if shared_map.is_none() {
                match SharedTelemetryMap::connect() {
                    Ok(map) => {
                        shared_map = Some(map);
                        runtime.plugin_installed.store(true, Ordering::Relaxed);
                        runtime.bridge_connected.store(false, Ordering::Relaxed);
                        last_timestamp = None;
                    }
                    Err(_) => {
                        runtime.plugin_installed.store(false, Ordering::Relaxed);
                        runtime.bridge_connected.store(false, Ordering::Relaxed);
                        emit_frontend_payload(
                            &app,
                            &mut last_payload,
                            FrontendTelemetryPayload {
                                speed: 0.0,
                                rpm: 0.0,
                                gear: "N".to_string(),
                                fuel: 0.0,
                                fuel_capacity: 0.0,
                                engine_on: false,
                                timestamp: 0,
                                paused: false,
                                plugin_installed: false,
                                sdk_connected: false,
                            },
                        );
                        thread::sleep(RECONNECT_INTERVAL);
                        continue;
                    }
                }
            }

            match shared_map.as_ref().unwrap().read_snapshot() {
                Ok(Some(snapshot)) => {
                    runtime.plugin_installed.store(true, Ordering::Relaxed);
                    if let Err(error) = logbook::process_snapshot(runtime.as_ref(), snapshot) {
                        crate::dev_log!("[career] telemetry logbook sync failed: {}", error);
                    }

                    let sdk_connected =
                        last_timestamp.is_some() && last_timestamp != Some(snapshot.timestamp);
                    runtime.bridge_connected.store(sdk_connected, Ordering::Relaxed);

                    emit_frontend_payload(
                        &app,
                        &mut last_payload,
                        snapshot.into_frontend_payload(true, sdk_connected),
                    );

                    last_timestamp = Some(snapshot.timestamp);
                    thread::sleep(POLL_INTERVAL);
                }
                Ok(None) => {
                    thread::sleep(Duration::from_millis(16));
                }
                Err(_) => {
                    shared_map = None;
                    last_timestamp = None;
                    runtime.plugin_installed.store(false, Ordering::Relaxed);
                    runtime.bridge_connected.store(false, Ordering::Relaxed);

                    emit_frontend_payload(
                        &app,
                        &mut last_payload,
                        FrontendTelemetryPayload {
                            speed: 0.0,
                            rpm: 0.0,
                            gear: "N".to_string(),
                            fuel: 0.0,
                            fuel_capacity: 0.0,
                            engine_on: false,
                            timestamp: 0,
                            paused: false,
                            plugin_installed: false,
                            sdk_connected: false,
                        },
                    );

                    thread::sleep(RECONNECT_INTERVAL);
                }
            }
        }

        runtime.plugin_installed.store(false, Ordering::Relaxed);
        runtime.bridge_connected.store(false, Ordering::Relaxed);
    }

    fn emit_frontend_payload(
        app: &AppHandle,
        last_payload: &mut Option<FrontendTelemetryPayload>,
        payload: FrontendTelemetryPayload,
    ) {
        if last_payload.as_ref() == Some(&payload) {
            return;
        }

        *last_payload = Some(payload.clone());
        let _ = app.emit("telemetry:update", payload);
    }

    fn change_token(zone: Zone1) -> u64 {
        if zone.render_timestamp != 0 {
            zone.render_timestamp
        } else if zone.simulation_timestamp != 0 {
            zone.simulation_timestamp
        } else {
            zone.timestamp
        }
    }

    fn format_gear(gear: i32) -> String {
        match gear {
            value if value < 0 => format!("R{}", value.abs()),
            0 => "N".to_string(),
            value => value.to_string(),
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(target_os = "windows")]
pub use platform::{start_frontend_telemetry_bridge, start_terminal_telemetry_loop};

#[cfg(not(target_os = "windows"))]
pub fn start_terminal_telemetry_loop() {}

#[cfg(not(target_os = "windows"))]
pub fn start_frontend_telemetry_bridge(
    _app: tauri::AppHandle,
    _runtime: std::sync::Arc<crate::state::CareerRuntime>,
) {
}
