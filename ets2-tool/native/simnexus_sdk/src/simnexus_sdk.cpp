#include <windows.h>

#include <cstdint>
#include <cstring>

#include "scssdk.h"
#include "scssdk_telemetry.h"
#include "scssdk_telemetry_channel.h"
#include "scssdk_telemetry_event.h"
#include "common/scssdk_telemetry_common_channels.h"
#include "common/scssdk_telemetry_common_configs.h"
#include "common/scssdk_telemetry_truck_common_channels.h"
#include "simnexus_telemetry_bridge.hpp"

namespace {

using simnexus::TelemetryBridgeHeader;
using simnexus::TelemetryData;

scs_log_t g_game_log = nullptr;
HANDLE g_mapping = nullptr;
TelemetryBridgeHeader* g_header = nullptr;
TelemetryData* g_payload = nullptr;
TelemetryData g_state{};

void log_message(const char* text) {
  if (g_game_log != nullptr) {
    g_game_log(SCS_LOG_TYPE_message, text);
  }
}

void log_error(const char* text) {
  if (g_game_log != nullptr) {
    g_game_log(SCS_LOG_TYPE_error, text);
  }
}

bool initialize_shared_memory() {
  const DWORD mapping_size =
      static_cast<DWORD>(sizeof(TelemetryBridgeHeader) + sizeof(TelemetryData));

  g_mapping = CreateFileMappingW(
      INVALID_HANDLE_VALUE,
      nullptr,
      PAGE_READWRITE,
      0,
      mapping_size,
      simnexus::kSharedMemoryName);

  if (g_mapping == nullptr) {
    log_error("[SimNexus] CreateFileMappingW failed");
    return false;
  }

  auto* base = static_cast<std::uint8_t*>(
      MapViewOfFile(g_mapping, FILE_MAP_ALL_ACCESS, 0, 0, mapping_size));
  if (base == nullptr) {
    log_error("[SimNexus] MapViewOfFile failed");
    CloseHandle(g_mapping);
    g_mapping = nullptr;
    return false;
  }

  g_header = reinterpret_cast<TelemetryBridgeHeader*>(base);
  g_payload = reinterpret_cast<TelemetryData*>(base + sizeof(TelemetryBridgeHeader));

  std::memcpy(g_header->magic, simnexus::kBridgeMagic, sizeof(g_header->magic));
  g_header->abi_version = simnexus::kBridgeAbiVersion;
  g_header->payload_size = static_cast<std::uint32_t>(sizeof(TelemetryData));
  g_header->sequence = 0;

  std::memset(&g_state, 0, sizeof(g_state));
  std::memset(g_payload, 0, sizeof(TelemetryData));

  log_message("[SimNexus] Shared memory initialized");
  return true;
}

void shutdown_shared_memory() {
  if (g_payload != nullptr) {
    g_payload = nullptr;
  }

  if (g_header != nullptr) {
    UnmapViewOfFile(g_header);
    g_header = nullptr;
  }

  if (g_mapping != nullptr) {
    CloseHandle(g_mapping);
    g_mapping = nullptr;
  }
}

void publish_snapshot() {
  if (g_header == nullptr || g_payload == nullptr) {
    return;
  }

  InterlockedIncrement64(&g_header->sequence);
  MemoryBarrier();
  std::memcpy(g_payload, &g_state, sizeof(TelemetryData));
  MemoryBarrier();
  InterlockedIncrement64(&g_header->sequence);
}

SCSAPI_VOID telemetry_store_float(
    const scs_string_t,
    const scs_u32_t,
    const scs_value_t* const value,
    const scs_context_t context) {
  if (value == nullptr || context == nullptr) {
    return;
  }

  *static_cast<float*>(context) = value->value_float.value;
}

SCSAPI_VOID telemetry_store_double(
    const scs_string_t,
    const scs_u32_t,
    const scs_value_t* const value,
    const scs_context_t context) {
  if (value == nullptr || context == nullptr) {
    return;
  }

  *static_cast<double*>(context) = value->value_double.value;
}

SCSAPI_VOID telemetry_store_s32(
    const scs_string_t,
    const scs_u32_t,
    const scs_value_t* const value,
    const scs_context_t context) {
  if (value == nullptr || context == nullptr) {
    return;
  }

  *static_cast<std::int32_t*>(context) = value->value_s32.value;
}

SCSAPI_VOID telemetry_store_speed(
    const scs_string_t,
    const scs_u32_t,
    const scs_value_t* const value,
    const scs_context_t) {
  if (value == nullptr) {
    return;
  }

  g_state.speed_kph = static_cast<double>(value->value_float.value) * 3.6;
}

SCSAPI_VOID telemetry_configuration(
    const scs_event_t,
    const void* const event_info,
    const scs_context_t) {
  const auto* configuration =
      static_cast<const scs_telemetry_configuration_t*>(event_info);
  if (configuration == nullptr || configuration->attributes == nullptr) {
    return;
  }

  for (const scs_named_value_t* attribute = configuration->attributes;
       attribute->name != nullptr;
       ++attribute) {
    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_fuel_capacity) == 0 &&
        attribute->value.type == SCS_VALUE_TYPE_float) {
      g_state.fuel_capacity_liters = attribute->value.value_float.value;
    }
  }
}

SCSAPI_VOID telemetry_game_started(
    const scs_event_t,
    const void* const,
    const scs_context_t) {
  g_state.paused = 0;
}

SCSAPI_VOID telemetry_game_paused(
    const scs_event_t,
    const void* const,
    const scs_context_t) {
  g_state.paused = 1;
}

SCSAPI_VOID telemetry_frame_end(
    const scs_event_t,
    const void* const,
    const scs_context_t) {
  ++g_state.frame_id;
  g_state.simulation_timestamp = static_cast<std::uint64_t>(GetTickCount64());
  publish_snapshot();
}

bool register_telemetry_channels(const scs_telemetry_init_params_v100_t* const params) {
  const bool speed_registered =
      params->register_for_channel(
          SCS_TELEMETRY_TRUCK_CHANNEL_speed,
          SCS_U32_NIL,
          SCS_VALUE_TYPE_float,
          SCS_TELEMETRY_CHANNEL_FLAG_none,
          telemetry_store_speed,
          nullptr) == SCS_RESULT_ok;

  const bool rpm_registered =
      params->register_for_channel(
          SCS_TELEMETRY_TRUCK_CHANNEL_engine_rpm,
          SCS_U32_NIL,
          SCS_VALUE_TYPE_float,
          SCS_TELEMETRY_CHANNEL_FLAG_none,
          telemetry_store_double,
          &g_state.engine_rpm) == SCS_RESULT_ok;

  const bool gear_registered =
      params->register_for_channel(
          SCS_TELEMETRY_TRUCK_CHANNEL_engine_gear,
          SCS_U32_NIL,
          SCS_VALUE_TYPE_s32,
          SCS_TELEMETRY_CHANNEL_FLAG_none,
          telemetry_store_s32,
          &g_state.gear) == SCS_RESULT_ok;

  const bool fuel_registered =
      params->register_for_channel(
          SCS_TELEMETRY_TRUCK_CHANNEL_fuel,
          SCS_U32_NIL,
          SCS_VALUE_TYPE_float,
          SCS_TELEMETRY_CHANNEL_FLAG_none,
          telemetry_store_float,
          &g_state.fuel_liters) == SCS_RESULT_ok;

  const bool odometer_registered =
      params->register_for_channel(
          SCS_TELEMETRY_TRUCK_CHANNEL_odometer,
          SCS_U32_NIL,
          SCS_VALUE_TYPE_double,
          SCS_TELEMETRY_CHANNEL_FLAG_none,
          telemetry_store_double,
          &g_state.odometer_km) == SCS_RESULT_ok;

  const bool scale_registered =
      params->register_for_channel(
          SCS_TELEMETRY_CHANNEL_local_scale,
          SCS_U32_NIL,
          SCS_VALUE_TYPE_float,
          SCS_TELEMETRY_CHANNEL_FLAG_none,
          telemetry_store_float,
          &g_state.map_scale) == SCS_RESULT_ok;

  return speed_registered && rpm_registered && gear_registered && fuel_registered &&
         odometer_registered && scale_registered;
}

bool register_telemetry_events(const scs_telemetry_init_params_v100_t* const params) {
  const bool configuration_registered =
      params->register_for_event(
          SCS_TELEMETRY_EVENT_configuration,
          telemetry_configuration,
          nullptr) == SCS_RESULT_ok;

  const bool started_registered =
      params->register_for_event(
          SCS_TELEMETRY_EVENT_started,
          telemetry_game_started,
          nullptr) == SCS_RESULT_ok;

  const bool paused_registered =
      params->register_for_event(
          SCS_TELEMETRY_EVENT_paused,
          telemetry_game_paused,
          nullptr) == SCS_RESULT_ok;

  const bool frame_end_registered =
      params->register_for_event(
          SCS_TELEMETRY_EVENT_frame_end,
          telemetry_frame_end,
          nullptr) == SCS_RESULT_ok;

  return configuration_registered && started_registered && paused_registered &&
         frame_end_registered;
}

} // namespace

extern "C" __declspec(dllexport) SCSAPI_RESULT scs_telemetry_init(
    const scs_u32_t version,
    const scs_telemetry_init_params_t* const params) {
  if (params == nullptr) {
    return SCS_RESULT_invalid_parameter;
  }

  if (SCS_GET_MAJOR_VERSION(version) != SCS_GET_MAJOR_VERSION(SCS_TELEMETRY_VERSION_1_00)) {
    return SCS_RESULT_unsupported;
  }

  const auto* version_params =
      static_cast<const scs_telemetry_init_params_v100_t*>(params);

  g_game_log = version_params->common.log;
  log_message("[SimNexus] Loading telemetry bridge");

  if (!initialize_shared_memory()) {
    return SCS_RESULT_generic_error;
  }

  if (!register_telemetry_events(version_params) ||
      !register_telemetry_channels(version_params)) {
    shutdown_shared_memory();
    log_error("[SimNexus] Failed to register telemetry callbacks");
    return SCS_RESULT_generic_error;
  }

  log_message("[SimNexus] Telemetry bridge active");
  return SCS_RESULT_ok;
}

extern "C" __declspec(dllexport) SCSAPI_VOID scs_telemetry_shutdown(void) {
  log_message("[SimNexus] Telemetry bridge shutting down");
  shutdown_shared_memory();
  g_game_log = nullptr;
}
