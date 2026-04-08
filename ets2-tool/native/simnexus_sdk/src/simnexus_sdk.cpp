#include <windows.h>

#include <cstdint>
#include <cstring>
#include <cstdio>
#include "scssdk.h"
#pragma push_macro("SCSAPI_RESULT")
#pragma push_macro("SCSAPI_VOID")
#undef SCSAPI_RESULT
#undef SCSAPI_VOID
#define SCSAPI_RESULT __declspec(dllexport) scs_result_t SCSAPIFUNC
#define SCSAPI_VOID __declspec(dllexport) void SCSAPIFUNC
#include "scssdk_telemetry.h"
#pragma pop_macro("SCSAPI_VOID")
#pragma pop_macro("SCSAPI_RESULT")
#include "scssdk_telemetry_channel.h"
#include "scssdk_telemetry_event.h"
#include "common/scssdk_telemetry_common_channels.h"
#include "common/scssdk_telemetry_common_gameplay_events.h"
#include "common/scssdk_telemetry_common_configs.h"
#include "common/scssdk_telemetry_job_common_channels.h"
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
std::uint8_t g_last_job_event = 0;
std::uint64_t g_last_job_hash = 0;

void clear_text(char* dst, const std::size_t size) {
  if (dst == nullptr || size == 0) return;
  std::memset(dst, 0, size);
}

void store_text(char* dst, const std::size_t size, const char* src) {
  if (dst == nullptr || size == 0) return;
  if (src == nullptr) {
    clear_text(dst, size);
    return;
  }
  std::strncpy(dst, src, size - 1);
  dst[size - 1] = '\0';
}

std::uint64_t fnv1a64(const char* text) {
  if (text == nullptr) return 0;
  std::uint64_t hash = 14695981039346656037ull;
  for (const unsigned char* p = reinterpret_cast<const unsigned char*>(text); *p != 0; ++p) {
    hash ^= static_cast<std::uint64_t>(*p);
    hash *= 1099511628211ull;
  }
  return hash;
}

void log_message(const char* text);

void update_job_id() {
  char buffer[512] = {};
  std::snprintf(
      buffer,
      sizeof(buffer),
      "%s|%s|%s|%s|%s|%lld|%u|%.0f|%u",
      g_state.source_city,
      g_state.destination_city,
      g_state.source_company,
      g_state.destination_company,
      g_state.cargo,
      static_cast<long long>(g_state.job_income),
      static_cast<unsigned>(g_state.job_delivery_time_min),
      g_state.job_planned_distance_km,
      static_cast<unsigned>(g_state.job_special));
  const std::uint64_t hash = fnv1a64(buffer);
  char id[64] = {};
  std::snprintf(id, sizeof(id), "job-%016llx", static_cast<unsigned long long>(hash));
  store_text(g_state.job_id, sizeof(g_state.job_id), id);

  if (hash != 0 && hash != g_last_job_hash) {
    g_last_job_hash = hash;
    char message[512] = {};
    std::snprintf(
        message,
        sizeof(message),
        "[SimNexus] Job detected: id=%s route=%s->%s cargo=%s income=%lld planned_km=%.1f delivery_min=%u market=%s special=%u",
        g_state.job_id,
        g_state.source_city,
        g_state.destination_city,
        g_state.cargo,
        static_cast<long long>(g_state.job_income),
        g_state.job_planned_distance_km,
        static_cast<unsigned>(g_state.job_delivery_time_min),
        g_state.job_market,
        static_cast<unsigned>(g_state.job_special));
    log_message(message);
  }
}

void clear_job_state() {
  g_state.job_active = 0;
  g_state.job_special = 0;
  g_state.job_cargo_loaded = 0;
  g_state.job_event = 0;
  g_state.job_income = 0;
  g_state.job_delivery_time_min = 0;
  g_state.job_planned_distance_km = 0.0;
  g_state.job_cargo_damage = 0.0;
  clear_text(g_state.job_id, sizeof(g_state.job_id));
  clear_text(g_state.source_city, sizeof(g_state.source_city));
  clear_text(g_state.destination_city, sizeof(g_state.destination_city));
  clear_text(g_state.source_company, sizeof(g_state.source_company));
  clear_text(g_state.destination_company, sizeof(g_state.destination_company));
  clear_text(g_state.cargo, sizeof(g_state.cargo));
  clear_text(g_state.job_market, sizeof(g_state.job_market));
  g_last_job_event = 0;
  g_last_job_hash = 0;
}

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
  g_state.payload_revision = 2;
  g_state.payload_reserved = 0;
  g_last_job_event = 0;

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

SCSAPI_VOID telemetry_store_float_to_double(
    const scs_string_t,
    const scs_u32_t,
    const scs_value_t* const value,
    const scs_context_t context) {
  if (value == nullptr || context == nullptr) {
    return;
  }

  *static_cast<double*>(context) = static_cast<double>(value->value_float.value);
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

SCSAPI_VOID telemetry_store_u32(
    const scs_string_t,
    const scs_u32_t,
    const scs_value_t* const value,
    const scs_context_t context) {
  if (value == nullptr || context == nullptr) {
    return;
  }

  *static_cast<std::uint32_t*>(context) = value->value_u32.value;
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

  if (std::strcmp(configuration->id, SCS_TELEMETRY_CONFIG_job) != 0) {
    return;
  }

  // When job config has an empty attribute set, there is no active job.
  bool has_any_attribute = false;
  for (const scs_named_value_t* attribute = configuration->attributes;
       attribute->name != nullptr;
       ++attribute) {
    has_any_attribute = true;
    break;
  }

  if (!has_any_attribute) {
    clear_job_state();
    return;
  }

  g_state.job_active = 1;
  g_state.job_event = 0;

  for (const scs_named_value_t* attribute = configuration->attributes;
       attribute->name != nullptr;
       ++attribute) {
    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_source_city) == 0 &&
        attribute->value.type == SCS_VALUE_TYPE_string) {
      store_text(g_state.source_city, sizeof(g_state.source_city), attribute->value.value_string.value);
      continue;
    }

    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_destination_city) == 0 &&
        attribute->value.type == SCS_VALUE_TYPE_string) {
      store_text(g_state.destination_city, sizeof(g_state.destination_city), attribute->value.value_string.value);
      continue;
    }

    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_source_company) == 0 &&
        attribute->value.type == SCS_VALUE_TYPE_string) {
      store_text(g_state.source_company, sizeof(g_state.source_company), attribute->value.value_string.value);
      continue;
    }

    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_destination_company) == 0 &&
        attribute->value.type == SCS_VALUE_TYPE_string) {
      store_text(g_state.destination_company, sizeof(g_state.destination_company), attribute->value.value_string.value);
      continue;
    }

    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_cargo) == 0 &&
        attribute->value.type == SCS_VALUE_TYPE_string) {
      store_text(g_state.cargo, sizeof(g_state.cargo), attribute->value.value_string.value);
      continue;
    }

    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_income) == 0) {
      if (attribute->value.type == SCS_VALUE_TYPE_s64) {
        g_state.job_income = attribute->value.value_s64.value;
      } else if (attribute->value.type == SCS_VALUE_TYPE_u64) {
        g_state.job_income = static_cast<std::int64_t>(attribute->value.value_u64.value);
      } else if (attribute->value.type == SCS_VALUE_TYPE_s32) {
        g_state.job_income = static_cast<std::int64_t>(attribute->value.value_s32.value);
      } else if (attribute->value.type == SCS_VALUE_TYPE_u32) {
        g_state.job_income = static_cast<std::int64_t>(attribute->value.value_u32.value);
      }
      continue;
    }

    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_delivery_time) == 0) {
      if (attribute->value.type == SCS_VALUE_TYPE_u32) {
        g_state.job_delivery_time_min = attribute->value.value_u32.value;
      } else if (attribute->value.type == SCS_VALUE_TYPE_s32) {
        g_state.job_delivery_time_min = static_cast<std::uint32_t>(attribute->value.value_s32.value);
      }
      continue;
    }

    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_planned_distance_km) == 0) {
      if (attribute->value.type == SCS_VALUE_TYPE_float) {
        g_state.job_planned_distance_km = static_cast<double>(attribute->value.value_float.value);
      } else if (attribute->value.type == SCS_VALUE_TYPE_double) {
        g_state.job_planned_distance_km = attribute->value.value_double.value;
      }
      continue;
    }

    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_job_market) == 0) {
      if (attribute->value.type == SCS_VALUE_TYPE_string) {
        store_text(g_state.job_market, sizeof(g_state.job_market), attribute->value.value_string.value);
      } else if (attribute->value.type == SCS_VALUE_TYPE_u32) {
        char buffer[32] = {};
        std::snprintf(buffer, sizeof(buffer), "%u", static_cast<unsigned>(attribute->value.value_u32.value));
        store_text(g_state.job_market, sizeof(g_state.job_market), buffer);
      } else if (attribute->value.type == SCS_VALUE_TYPE_s32) {
        char buffer[32] = {};
        std::snprintf(buffer, sizeof(buffer), "%d", static_cast<int>(attribute->value.value_s32.value));
        store_text(g_state.job_market, sizeof(g_state.job_market), buffer);
      }
      continue;
    }

    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_special_job) == 0 &&
        attribute->value.type == SCS_VALUE_TYPE_bool) {
      g_state.job_special = attribute->value.value_bool.value ? 1 : 0;
      continue;
    }

    if (std::strcmp(attribute->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_is_cargo_loaded) == 0 &&
        attribute->value.type == SCS_VALUE_TYPE_bool) {
      g_state.job_cargo_loaded = attribute->value.value_bool.value ? 1 : 0;
      continue;
    }
  }

  update_job_id();
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

  // Reset one-shot event after it has been published once.
  if (g_last_job_event != 0) {
    g_state.job_event = 0;
    g_last_job_event = 0;
  }

  publish_snapshot();
}

SCSAPI_VOID telemetry_gameplay_event(
    const scs_event_t,
    const void* const event_info,
    const scs_context_t) {
  const auto* gameplay = static_cast<const scs_telemetry_gameplay_event_t*>(event_info);
  if (gameplay == nullptr || gameplay->id == nullptr) return;

  if (std::strcmp(gameplay->id, SCS_TELEMETRY_GAMEPLAY_EVENT_job_delivered) == 0) {
    g_state.job_event = 1;
  } else if (std::strcmp(gameplay->id, SCS_TELEMETRY_GAMEPLAY_EVENT_job_cancelled) == 0) {
    g_state.job_event = 2;
  } else {
    return;
  }

  if (gameplay->attributes == nullptr) return;
  for (const scs_named_value_t* attribute = gameplay->attributes;
       attribute->name != nullptr;
       ++attribute) {
    if (std::strcmp(attribute->name, SCS_TELEMETRY_GAMEPLAY_EVENT_ATTRIBUTE_cargo_damage) == 0) {
      if (attribute->value.type == SCS_VALUE_TYPE_float) {
        g_state.job_cargo_damage = static_cast<double>(attribute->value.value_float.value);
      } else if (attribute->value.type == SCS_VALUE_TYPE_double) {
        g_state.job_cargo_damage = attribute->value.value_double.value;
      }
    }
  }

  // Track as one-shot event: reset after being published once.
  g_last_job_event = g_state.job_event;
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
          telemetry_store_float_to_double,
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

  const bool game_time_registered =
      params->register_for_channel(
          SCS_TELEMETRY_CHANNEL_game_time,
          SCS_U32_NIL,
          SCS_VALUE_TYPE_u32,
          SCS_TELEMETRY_CHANNEL_FLAG_none,
          telemetry_store_u32,
          &g_state.game_time_min) == SCS_RESULT_ok;

  const bool cargo_damage_registered =
      params->register_for_channel(
          SCS_TELEMETRY_JOB_CHANNEL_cargo_damage,
          SCS_U32_NIL,
          SCS_VALUE_TYPE_float,
          SCS_TELEMETRY_CHANNEL_FLAG_none,
          telemetry_store_float_to_double,
          &g_state.job_cargo_damage) == SCS_RESULT_ok;

  return speed_registered && rpm_registered && gear_registered && fuel_registered &&
         odometer_registered && scale_registered && game_time_registered &&
         cargo_damage_registered;
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

  const bool gameplay_registered =
      params->register_for_event(
          SCS_TELEMETRY_EVENT_gameplay,
          telemetry_gameplay_event,
          nullptr) == SCS_RESULT_ok;

  return configuration_registered && started_registered && paused_registered &&
         frame_end_registered && gameplay_registered;
}

} // namespace

SCSSDK_HEADER

__declspec(dllexport) SCSAPI_RESULT scs_telemetry_init(
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

__declspec(dllexport) SCSAPI_VOID scs_telemetry_shutdown(void) {
  log_message("[SimNexus] Telemetry bridge shutting down");
  shutdown_shared_memory();
  g_game_log = nullptr;
}

SCSSDK_FOOTER
