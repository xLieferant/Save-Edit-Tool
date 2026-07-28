#include <windows.h>

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "simnexus_telemetry_bridge.hpp"

namespace {

using simnexus::TelemetryBridgeHeader;
using simnexus::TelemetryData;

std::uint64_t unix_timestamp_ms() {
  FILETIME file_time{};
  GetSystemTimePreciseAsFileTime(&file_time);
  ULARGE_INTEGER value{};
  value.LowPart = file_time.dwLowDateTime;
  value.HighPart = file_time.dwHighDateTime;
  constexpr std::uint64_t kWindowsToUnixEpoch100ns = 116444736000000000ull;
  return (value.QuadPart - kWindowsToUnixEpoch100ns) / 10'000ull;
}

template <std::size_t Size>
void store_text(char (&target)[Size], const char* text) {
  std::memset(target, 0, Size);
  if (text == nullptr) return;
  const std::size_t count = std::min(Size - 1, std::strlen(text));
  std::memcpy(target, text, count);
}

void publish(TelemetryBridgeHeader* header, TelemetryData* target, const TelemetryData& source) {
  InterlockedIncrement64(reinterpret_cast<volatile LONG64*>(&header->sequence));
  MemoryBarrier();
  std::memcpy(target, &source, sizeof(source));
  MemoryBarrier();
  InterlockedIncrement64(reinterpret_cast<volatile LONG64*>(&header->sequence));
}

bool verify(const TelemetryBridgeHeader* header, const TelemetryData* payload) {
  const auto before = InterlockedCompareExchange64(
      reinterpret_cast<volatile LONG64*>(const_cast<std::int64_t*>(&header->sequence)), 0, 0);
  if ((before & 1) != 0) return false;
  TelemetryData copy{};
  std::memcpy(&copy, payload, sizeof(copy));
  MemoryBarrier();
  const auto after = InterlockedCompareExchange64(
      reinterpret_cast<volatile LONG64*>(const_cast<std::int64_t*>(&header->sequence)), 0, 0);
  return before == after && (after & 1) == 0 && copy.payload_revision == 3 &&
         copy.plugin_initialized == 1 && copy.sdk_connected == 1 && copy.job_active == 1 &&
         std::strcmp(copy.source_city, "L\xC3\xBC" "beck") == 0 &&
         std::strcmp(copy.destination_city, "Hamburg") == 0 &&
         std::strcmp(copy.cargo, "Test Cargo") == 0;
}

} // namespace

int wmain(int argc, wchar_t** argv) {
  const int seconds = argc > 1 ? std::clamp(_wtoi(argv[1]), 1, 300) : 30;
  constexpr DWORD mapping_size = sizeof(TelemetryBridgeHeader) + sizeof(TelemetryData);
  HANDLE mapping = CreateFileMappingW(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE, 0,
                                      mapping_size, simnexus::kSharedMemoryName);
  if (mapping == nullptr) {
    std::fprintf(stderr, "FAIL CreateFileMappingW error=%lu\n", GetLastError());
    return EXIT_FAILURE;
  }
  auto* base = static_cast<std::uint8_t*>(
      MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, mapping_size));
  if (base == nullptr) {
    std::fprintf(stderr, "FAIL MapViewOfFile error=%lu\n", GetLastError());
    CloseHandle(mapping);
    return EXIT_FAILURE;
  }

  auto* header = reinterpret_cast<TelemetryBridgeHeader*>(base);
  auto* payload = reinterpret_cast<TelemetryData*>(base + sizeof(TelemetryBridgeHeader));
  std::memset(base, 0, mapping_size);
  std::memcpy(header->magic, simnexus::kBridgeMagic, sizeof(header->magic));
  header->protocol_version = simnexus::kBridgeProtocolVersion;
  header->payload_size = sizeof(TelemetryData);

  TelemetryData state{};
  state.payload_revision = simnexus::kPayloadRevision;
  state.process_id = GetCurrentProcessId();
  state.plugin_initialized = 1;
  state.sdk_connected = 1;
  state.telemetry_active = 1;
  state.job_active = 1;
  state.engine_enabled = 1;
  state.speed_kph = 82.5;
  state.engine_rpm = 1'250.0;
  state.odometer_km = 123'456.75;
  state.fuel_liters = 415.0f;
  state.fuel_capacity_liters = 600.0f;
  state.gear = 10;
  state.job_income = 42'000;
  state.job_delivery_time_min = 600;
  state.game_time_min = 120;
  state.job_planned_distance_km = 815.0;
  store_text(state.build_id, "selftest-v3");
  store_text(state.dll_version, "3.0.0");
  store_text(state.game_id, "eut2");
  store_text(state.dll_path, "<synthetic-selftest>");
  store_text(state.job_id, "job-selftest-v3");
  store_text(state.source_city, "L\xC3\xBC" "beck");
  store_text(state.destination_city, "Hamburg");
  store_text(state.source_company, "Test Source");
  store_text(state.destination_company, "Test Destination");
  store_text(state.cargo, "Test Cargo");
  store_text(state.job_market, "freight_market");

  std::printf("SimNexus V3 self-test mapping=%ls seconds=%d payload=%zu\n",
              simnexus::kSharedMemoryName, seconds, sizeof(TelemetryData));
  bool valid = true;
  const DWORD iterations = static_cast<DWORD>(seconds * 1000) / simnexus::kHeartbeatIntervalMs;
  for (DWORD index = 0; index < iterations; ++index) {
    state.heartbeat_timestamp_ms = unix_timestamp_ms();
    state.telemetry_timestamp_ms = state.heartbeat_timestamp_ms;
    ++state.frame_id;
    publish(header, payload, state);
    valid = valid && verify(header, payload);
    Sleep(simnexus::kHeartbeatIntervalMs);
  }

  UnmapViewOfFile(base);
  CloseHandle(mapping);
  if (!valid) {
    std::fprintf(stderr, "FAIL inconsistent or invalid shared-memory snapshot\n");
    return EXIT_FAILURE;
  }
  std::printf("PASS frames=%llu source=Luebeck destination=Hamburg cargo=Test Cargo\n",
              static_cast<unsigned long long>(state.frame_id));
  return EXIT_SUCCESS;
}
