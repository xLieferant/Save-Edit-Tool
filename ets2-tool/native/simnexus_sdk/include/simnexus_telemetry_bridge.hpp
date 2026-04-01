#pragma once

#include <cstdint>

namespace simnexus {

inline constexpr wchar_t kSharedMemoryName[] = L"Local\\SimNexusTelemetry";
inline constexpr char kBridgeMagic[8] = {'S', 'N', 'X', 'T', 'L', 'M', '0', '1'};
inline constexpr std::uint32_t kBridgeAbiVersion = 1;

struct TelemetryBridgeHeader {
  char magic[8];
  std::uint32_t abi_version;
  std::uint32_t payload_size;
  volatile std::int64_t sequence;
};

struct TelemetryData {
  std::uint64_t frame_id;
  std::uint64_t simulation_timestamp;
  std::uint32_t payload_revision;
  std::uint32_t payload_reserved;
  double speed_kph;
  double engine_rpm;
  double odometer_km;
  float fuel_liters;
  float fuel_capacity_liters;
  float map_scale;
  std::int32_t gear;
  std::uint8_t paused;
  std::uint8_t job_active;
  std::uint8_t job_special;
  std::uint8_t job_cargo_loaded;
  std::uint8_t job_event;
  std::int64_t job_income;
  std::uint32_t job_delivery_time_min;
  std::uint32_t game_time_min;
  double job_planned_distance_km;
  double job_cargo_damage;
  char job_id[64];
  char source_city[64];
  char destination_city[64];
  char source_company[64];
  char destination_company[64];
  char cargo[64];
  char job_market[32];
  std::uint8_t reserved[8];
};

static_assert(sizeof(TelemetryBridgeHeader) == 24, "Unexpected bridge header size");
static_assert(sizeof(TelemetryData) == 528, "Unexpected telemetry payload size");

} // namespace simnexus
