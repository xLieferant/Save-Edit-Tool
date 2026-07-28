#pragma once

#include <cstdint>

namespace simnexus {

inline constexpr wchar_t kSharedMemoryName[] = L"Local\\SimNexusTelemetryV3";
inline constexpr wchar_t kLegacySharedMemoryName[] = L"Local\\SimNexusTelemetry";
inline constexpr char kBridgeMagic[8] = {'S', 'N', 'X', 'T', 'L', 'M', '0', '3'};
inline constexpr std::uint32_t kBridgeProtocolVersion = 3;
inline constexpr std::uint32_t kPayloadRevision = 3;
inline constexpr std::uint32_t kHeartbeatIntervalMs = 250;
inline constexpr std::uint32_t kHeartbeatStaleAfterMs = 2'000;

struct TelemetryBridgeHeader {
  char magic[8];
  std::uint32_t protocol_version;
  std::uint32_t payload_size;
  volatile std::int64_t sequence;
};

struct TelemetryData {
  std::uint64_t heartbeat_timestamp_ms;
  std::uint64_t telemetry_timestamp_ms;
  std::uint64_t frame_id;
  std::uint32_t payload_revision;
  std::uint32_t game_version;
  std::uint32_t telemetry_sdk_version;
  std::uint32_t process_id;

  std::uint8_t plugin_initialized;
  std::uint8_t sdk_connected;
  std::uint8_t telemetry_active;
  std::uint8_t game_paused;
  std::uint8_t job_active;
  std::uint8_t job_special;
  std::uint8_t job_cargo_loaded;
  std::uint8_t job_event;
  std::uint8_t engine_enabled;
  std::uint8_t mapping_preexisting;
  std::uint8_t telemetry_callback_seen;
  std::uint8_t job_config_seen;
  std::uint8_t reserved_flags[4];

  std::uint64_t job_event_sequence;

  double speed_kph;
  double engine_rpm;
  double odometer_km;
  float fuel_liters;
  float fuel_capacity_liters;
  float map_scale;
  std::int32_t gear;

  std::int64_t job_income;
  std::uint32_t job_delivery_time_min;
  std::uint32_t game_time_min;
  double job_planned_distance_km;
  double job_cargo_damage;

  char build_id[48];
  char dll_version[16];
  char game_id[16];
  char dll_path[512];

  char job_id[64];
  char source_city[64];
  char destination_city[64];
  char source_company[64];
  char destination_company[64];
  char cargo[64];
  char cargo_id[64];
  char source_city_id[64];
  char destination_city_id[64];
  char source_company_id[64];
  char destination_company_id[64];
  char job_market[32];

  std::uint8_t reserved[584];
};

static_assert(sizeof(TelemetryBridgeHeader) == 24, "Unexpected bridge header size");
static_assert(alignof(TelemetryBridgeHeader) == 8, "Unexpected bridge header alignment");
static_assert(sizeof(TelemetryData) == 2048, "Unexpected telemetry payload size");
static_assert(alignof(TelemetryData) == 8, "Unexpected telemetry payload alignment");

} // namespace simnexus
