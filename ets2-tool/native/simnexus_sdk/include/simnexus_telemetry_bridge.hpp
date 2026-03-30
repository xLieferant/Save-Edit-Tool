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
  double speed_kph;
  double engine_rpm;
  double odometer_km;
  float fuel_liters;
  float fuel_capacity_liters;
  float map_scale;
  std::int32_t gear;
  std::uint8_t paused;
  std::uint8_t reserved[3];
};

static_assert(sizeof(TelemetryBridgeHeader) == 24, "Unexpected bridge header size");
static_assert(sizeof(TelemetryData) == 56, "Unexpected telemetry payload size");

} // namespace simnexus
