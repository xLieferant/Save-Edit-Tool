use std::mem::{MaybeUninit, offset_of, size_of};
use std::ptr;
use std::sync::atomic::{AtomicI64, Ordering, fence};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const SHARED_MEMORY_NAME: &str = "Local\\SimNexusTelemetryV3";
pub(crate) const LEGACY_SHARED_MEMORY_NAME: &str = "Local\\SimNexusTelemetry";
pub(crate) const BRIDGE_MAGIC: [u8; 8] = *b"SNXTLM03";
pub(crate) const BRIDGE_PROTOCOL_VERSION: u32 = 3;
pub(crate) const PAYLOAD_REVISION: u32 = 3;
pub(crate) const HEARTBEAT_STALE_AFTER_MS: u64 = 2_000;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct BridgeHeader {
    pub(crate) magic: [u8; 8],
    pub(crate) protocol_version: u32,
    pub(crate) payload_size: u32,
    pub(crate) sequence: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct TelemetryDataV3 {
    pub(crate) heartbeat_timestamp_ms: u64,
    pub(crate) telemetry_timestamp_ms: u64,
    pub(crate) frame_id: u64,
    pub(crate) payload_revision: u32,
    pub(crate) game_version: u32,
    pub(crate) telemetry_sdk_version: u32,
    pub(crate) process_id: u32,

    pub(crate) plugin_initialized: u8,
    pub(crate) sdk_connected: u8,
    pub(crate) telemetry_active: u8,
    pub(crate) game_paused: u8,
    pub(crate) job_active: u8,
    pub(crate) job_special: u8,
    pub(crate) job_cargo_loaded: u8,
    pub(crate) job_event: u8,
    pub(crate) engine_enabled: u8,
    pub(crate) mapping_preexisting: u8,
    pub(crate) telemetry_callback_seen: u8,
    pub(crate) job_config_seen: u8,
    pub(crate) reserved_flags: [u8; 4],

    pub(crate) job_event_sequence: u64,

    pub(crate) speed_kph: f64,
    pub(crate) engine_rpm: f64,
    pub(crate) odometer_km: f64,
    pub(crate) fuel_liters: f32,
    pub(crate) fuel_capacity_liters: f32,
    pub(crate) map_scale: f32,
    pub(crate) gear: i32,

    pub(crate) job_income: i64,
    pub(crate) job_delivery_time_min: u32,
    pub(crate) game_time_min: u32,
    pub(crate) job_planned_distance_km: f64,
    pub(crate) job_cargo_damage: f64,

    pub(crate) build_id: [u8; 48],
    pub(crate) dll_version: [u8; 16],
    pub(crate) game_id: [u8; 16],
    pub(crate) dll_path: [u8; 512],

    pub(crate) job_id: [u8; 64],
    pub(crate) source_city: [u8; 64],
    pub(crate) destination_city: [u8; 64],
    pub(crate) source_company: [u8; 64],
    pub(crate) destination_company: [u8; 64],
    pub(crate) cargo: [u8; 64],
    pub(crate) cargo_id: [u8; 64],
    pub(crate) source_city_id: [u8; 64],
    pub(crate) destination_city_id: [u8; 64],
    pub(crate) source_company_id: [u8; 64],
    pub(crate) destination_company_id: [u8; 64],
    pub(crate) job_market: [u8; 32],

    pub(crate) reserved: [u8; 584],
}

impl Default for TelemetryDataV3 {
    fn default() -> Self {
        unsafe { MaybeUninit::<Self>::zeroed().assume_init() }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BridgeSnapshot {
    pub(crate) sequence: i64,
    pub(crate) payload: TelemetryDataV3,
}

pub(crate) fn bytes_to_string(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

pub(crate) fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub(crate) fn heartbeat_age_ms(payload: &TelemetryDataV3) -> u64 {
    unix_timestamp_ms().saturating_sub(payload.heartbeat_timestamp_ms)
}

pub(crate) fn validate_liveness(payload: &TelemetryDataV3) -> Result<u64, String> {
    if payload.payload_revision != PAYLOAD_REVISION {
        return Err(format!(
            "Payload revision mismatch: DLL={} App={}",
            payload.payload_revision, PAYLOAD_REVISION
        ));
    }
    if payload.plugin_initialized == 0 {
        return Err("DLL mapping exists, but plugin_initialized is false".to_string());
    }
    if payload.sdk_connected == 0 {
        return Err("DLL initialized, but SCS SDK is not connected".to_string());
    }
    if payload.heartbeat_timestamp_ms == 0 {
        return Err("DLL initialized, but heartbeat has never been written".to_string());
    }
    let age = heartbeat_age_ms(payload);
    if age > HEARTBEAT_STALE_AFTER_MS {
        return Err(format!("Heartbeat stale: last update {age} ms ago"));
    }
    Ok(age)
}

pub(crate) unsafe fn read_consistent(base: *const u8) -> Result<Option<BridgeSnapshot>, String> {
    if base.is_null() {
        return Err("Shared memory view is null".to_string());
    }
    let header_ptr = base.cast::<BridgeHeader>();
    let magic = unsafe { ptr::read_volatile(ptr::addr_of!((*header_ptr).magic)) };
    if magic != BRIDGE_MAGIC {
        return Err(format!(
            "Shared memory magic is invalid: expected={:?} actual={:?}",
            BRIDGE_MAGIC, magic
        ));
    }
    let protocol_version =
        unsafe { ptr::read_volatile(ptr::addr_of!((*header_ptr).protocol_version)) };
    if protocol_version != BRIDGE_PROTOCOL_VERSION {
        return Err(format!(
            "Shared memory protocol mismatch: DLL={} App={}",
            protocol_version, BRIDGE_PROTOCOL_VERSION
        ));
    }
    let payload_size = unsafe { ptr::read_volatile(ptr::addr_of!((*header_ptr).payload_size)) };
    if payload_size as usize != size_of::<TelemetryDataV3>() {
        return Err(format!(
            "Payload size mismatch: DLL={} App={}",
            payload_size,
            size_of::<TelemetryDataV3>()
        ));
    }

    let sequence_ptr = unsafe { ptr::addr_of!((*header_ptr).sequence).cast::<AtomicI64>() };
    let sequence_before = unsafe { (*sequence_ptr).load(Ordering::Acquire) };
    if sequence_before & 1 == 1 {
        return Ok(None);
    }

    let payload_ptr = unsafe { base.add(size_of::<BridgeHeader>()) };
    let mut bytes = [0u8; size_of::<TelemetryDataV3>()];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = unsafe { ptr::read_volatile(payload_ptr.add(index)) };
    }
    fence(Ordering::Acquire);
    let sequence_after = unsafe { (*sequence_ptr).load(Ordering::Acquire) };
    if sequence_before != sequence_after || sequence_after & 1 == 1 {
        return Ok(None);
    }

    let payload = unsafe { ptr::read_unaligned(bytes.as_ptr().cast::<TelemetryDataV3>()) };
    Ok(Some(BridgeSnapshot {
        sequence: sequence_after,
        payload,
    }))
}

pub(crate) fn layout_diagnostic() -> String {
    format!(
        "protocol={} header_size={} payload_size={} align={} off(heartbeat)={} off(telemetry_timestamp)={} off(frame_id)={} off(job_active)={} off(job_event)={} off(job_id)={}",
        BRIDGE_PROTOCOL_VERSION,
        size_of::<BridgeHeader>(),
        size_of::<TelemetryDataV3>(),
        std::mem::align_of::<TelemetryDataV3>(),
        offset_of!(TelemetryDataV3, heartbeat_timestamp_ms),
        offset_of!(TelemetryDataV3, telemetry_timestamp_ms),
        offset_of!(TelemetryDataV3, frame_id),
        offset_of!(TelemetryDataV3, job_active),
        offset_of!(TelemetryDataV3, job_event),
        offset_of!(TelemetryDataV3, job_id)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_text<const N: usize>(target: &mut [u8; N], value: &str) {
        let bytes = value.as_bytes();
        let count = bytes.len().min(N.saturating_sub(1));
        target[..count].copy_from_slice(&bytes[..count]);
    }

    fn synthetic_mapping(sequence: i64) -> Vec<u64> {
        let total_size = size_of::<BridgeHeader>() + size_of::<TelemetryDataV3>();
        let mut mapping = vec![0u64; total_size.div_ceil(size_of::<u64>())];
        let base = mapping.as_mut_ptr().cast::<u8>();
        let header = BridgeHeader {
            magic: BRIDGE_MAGIC,
            protocol_version: BRIDGE_PROTOCOL_VERSION,
            payload_size: size_of::<TelemetryDataV3>() as u32,
            sequence,
        };
        let mut payload = TelemetryDataV3 {
            heartbeat_timestamp_ms: unix_timestamp_ms(),
            telemetry_timestamp_ms: unix_timestamp_ms(),
            frame_id: 42,
            payload_revision: PAYLOAD_REVISION,
            plugin_initialized: 1,
            sdk_connected: 1,
            telemetry_active: 1,
            job_active: 1,
            ..Default::default()
        };
        write_text(&mut payload.source_city, "Lübeck");
        write_text(&mut payload.destination_city, "Hamburg");
        write_text(&mut payload.cargo, "Test Cargo");
        unsafe {
            ptr::write(base.cast::<BridgeHeader>(), header);
            ptr::write(
                base.add(size_of::<BridgeHeader>())
                    .cast::<TelemetryDataV3>(),
                payload,
            );
        }
        mapping
    }

    #[test]
    fn c_layout_matches_cpp_contract() {
        assert_eq!(size_of::<BridgeHeader>(), 24);
        assert_eq!(size_of::<TelemetryDataV3>(), 2048);
        assert_eq!(std::mem::align_of::<TelemetryDataV3>(), 8);
        assert_eq!(offset_of!(TelemetryDataV3, heartbeat_timestamp_ms), 0);
        assert_eq!(offset_of!(TelemetryDataV3, telemetry_timestamp_ms), 8);
        assert_eq!(offset_of!(TelemetryDataV3, frame_id), 16);
        assert_eq!(offset_of!(TelemetryDataV3, plugin_initialized), 40);
        assert_eq!(offset_of!(TelemetryDataV3, job_active), 44);
        assert_eq!(offset_of!(TelemetryDataV3, job_event), 47);
        assert_eq!(offset_of!(TelemetryDataV3, job_event_sequence), 56);
        assert_eq!(offset_of!(TelemetryDataV3, speed_kph), 64);
        assert_eq!(offset_of!(TelemetryDataV3, job_income), 104);
        assert_eq!(offset_of!(TelemetryDataV3, job_planned_distance_km), 120);
        assert_eq!(offset_of!(TelemetryDataV3, build_id), 136);
        assert_eq!(offset_of!(TelemetryDataV3, dll_path), 216);
        assert_eq!(offset_of!(TelemetryDataV3, job_id), 728);
    }

    #[test]
    fn reads_complete_even_sequence() {
        let mapping = synthetic_mapping(8);
        let snapshot = unsafe { read_consistent(mapping.as_ptr().cast::<u8>()) }
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.sequence, 8);
        assert_eq!(snapshot.payload.frame_id, 42);
        assert_eq!(bytes_to_string(&snapshot.payload.source_city), "Lübeck");
        assert_eq!(
            bytes_to_string(&snapshot.payload.destination_city),
            "Hamburg"
        );
        assert!(validate_liveness(&snapshot.payload).unwrap() <= 10);
    }

    #[test]
    fn rejects_in_progress_write() {
        let mapping = synthetic_mapping(9);
        let snapshot = unsafe { read_consistent(mapping.as_ptr().cast::<u8>()) }.unwrap();
        assert!(snapshot.is_none());
    }
}
