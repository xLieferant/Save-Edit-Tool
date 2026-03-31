#[cfg(target_os = "windows")]
mod platform {
    use std::ptr;
    use std::slice;
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::Memory::{
        FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, OpenFileMappingW,
        UnmapViewOfFile,
    };

    const SHARED_MEMORY_NAME: &str = "Local\\SCSTelemetry";
    const SHARED_MEMORY_SIZE: usize = 32 * 1024;
    const RAW_READ_SIZE: usize = 128;
    const TIMESTAMP_OFFSET: usize = 8;
    const SIMULATION_TIMESTAMP_OFFSET: usize = 16;
    const RENDER_TIMESTAMP_OFFSET: usize = 24;
    const POLL_INTERVAL: Duration = Duration::from_millis(500);
    const RECONNECT_INTERVAL: Duration = Duration::from_millis(1000);
    const STATIC_READ_THRESHOLD: u32 = 3;
    const NOT_AVAILABLE_MESSAGE: &str =
        "Telemetry not available. Is ETS2 running with the plugin?";

    #[derive(Clone, PartialEq, Eq)]
    struct DebugSample {
        raw_data: [u8; RAW_READ_SIZE],
        timestamp: u64,
        simulation_timestamp: u64,
        render_timestamp: u64,
        active_timestamp: u64,
    }

    struct SharedMemoryDebugMap {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
    }

    impl SharedMemoryDebugMap {
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

        fn read_sample(&self) -> DebugSample {
            let mut raw_data = [0u8; RAW_READ_SIZE];

            unsafe {
                let data = slice::from_raw_parts(self.view.Value as *const u8, RAW_READ_SIZE);
                raw_data.copy_from_slice(data);
            }

            let timestamp = read_u64_le(&raw_data, TIMESTAMP_OFFSET);
            let simulation_timestamp = read_u64_le(&raw_data, SIMULATION_TIMESTAMP_OFFSET);
            let render_timestamp = read_u64_le(&raw_data, RENDER_TIMESTAMP_OFFSET);

            DebugSample {
                raw_data,
                timestamp,
                simulation_timestamp,
                render_timestamp,
                active_timestamp: active_timestamp(
                    timestamp,
                    simulation_timestamp,
                    render_timestamp,
                ),
            }
        }
    }

    impl Drop for SharedMemoryDebugMap {
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

    pub fn start_telemetry_debug_thread() {
        if let Err(error) = thread::Builder::new()
            .name("scs-telemetry-debug".to_string())
            .spawn(debug_loop)
        {
            eprintln!("Failed to start telemetry debug thread: {error}");
        }
    }

    fn debug_loop() {
        let mut debug_map: Option<SharedMemoryDebugMap> = None;
        let mut previous_sample: Option<DebugSample> = None;
        let mut last_found_state: Option<bool> = None;
        let mut static_reads = 0u32;
        let mut static_message_logged = false;

        loop {
            if debug_map.is_none() {
                match SharedMemoryDebugMap::connect() {
                    Ok(map) => {
                        debug_map = Some(map);
                        previous_sample = None;
                        static_reads = 0;
                        static_message_logged = false;
                        if last_found_state != Some(true) {
                            println!("Shared Memory FOUND");
                        }
                        last_found_state = Some(true);
                    }
                    Err(_) => {
                        if last_found_state != Some(false) {
                            println!("Shared Memory NOT FOUND");
                            println!("{NOT_AVAILABLE_MESSAGE}");
                        }
                        last_found_state = Some(false);
                        thread::sleep(RECONNECT_INTERVAL);
                        continue;
                    }
                }
            }

            let sample = debug_map.as_ref().unwrap().read_sample();

            if previous_sample.is_none() {
                println!("Raw Data: {:?}", sample.raw_data);
                println!(
                    "Timestamp: {} | Simulation Timestamp: {} | Render Timestamp: {}",
                    sample.timestamp,
                    sample.simulation_timestamp,
                    sample.render_timestamp
                );
                previous_sample = Some(sample);
                thread::sleep(POLL_INTERVAL);
                continue;
            }

            let previous = previous_sample.as_ref().unwrap();
            let raw_changed = sample.raw_data != previous.raw_data;
            let timestamp_changed = sample.active_timestamp != previous.active_timestamp;

            println!(
                "Timestamp: {} | Raw changed: {} | Timestamp changed: {}",
                sample.active_timestamp, raw_changed, timestamp_changed
            );

            if raw_changed || timestamp_changed {
                static_reads = 0;
                static_message_logged = false;
            } else {
                static_reads += 1;
                if static_reads >= STATIC_READ_THRESHOLD && !static_message_logged {
                    println!("Shared Memory found but no data updates detected");
                    static_message_logged = true;
                }
            }

            previous_sample = Some(sample);
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn active_timestamp(timestamp: u64, simulation_timestamp: u64, render_timestamp: u64) -> u64 {
        if render_timestamp != 0 {
            render_timestamp
        } else if simulation_timestamp != 0 {
            simulation_timestamp
        } else {
            timestamp
        }
    }

    fn read_u64_le(bytes: &[u8; RAW_READ_SIZE], offset: usize) -> u64 {
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&bytes[offset..offset + 8]);
        u64::from_le_bytes(raw)
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[allow(dead_code)]
    unsafe fn _read_volatile_byte(ptr_base: *const u8, offset: usize) -> u8 {
        unsafe { ptr::read_volatile(ptr_base.add(offset)) }
    }
}

#[cfg(target_os = "windows")]
pub use platform::start_telemetry_debug_thread;

#[cfg(not(target_os = "windows"))]
pub fn start_telemetry_debug_thread() {}
