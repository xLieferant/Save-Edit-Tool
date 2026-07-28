#include <windows.h>

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>

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
#include "common/scssdk_telemetry_common_channels.h"
#include "common/scssdk_telemetry_common_configs.h"
#include "common/scssdk_telemetry_common_gameplay_events.h"
#include "common/scssdk_telemetry_job_common_channels.h"
#include "common/scssdk_telemetry_truck_common_channels.h"
#include "scssdk_telemetry_channel.h"
#include "scssdk_telemetry_event.h"
#include "simnexus_telemetry_bridge.hpp"

#ifndef SIMNEXUS_BUILD_ID
#define SIMNEXUS_BUILD_ID __DATE__ "-" __TIME__
#endif

#ifndef SIMNEXUS_DLL_VERSION
#define SIMNEXUS_DLL_VERSION "3.0.0"
#endif

namespace {

using simnexus::TelemetryBridgeHeader;
using simnexus::TelemetryData;

scs_log_t g_game_log = nullptr;
HMODULE g_module = nullptr;
HANDLE g_mapping = nullptr;
HANDLE g_heartbeat_stop = nullptr;
HANDLE g_heartbeat_thread = nullptr;
TelemetryBridgeHeader* g_header = nullptr;
TelemetryData* g_payload = nullptr;
TelemetryData g_state{};
SRWLOCK g_state_lock = SRWLOCK_INIT;
SRWLOCK g_publish_lock = SRWLOCK_INIT;
std::uint64_t g_last_job_hash = 0;

class StateLock final {
 public:
  StateLock() { AcquireSRWLockExclusive(&g_state_lock); }
  ~StateLock() { ReleaseSRWLockExclusive(&g_state_lock); }
  StateLock(const StateLock&) = delete;
  StateLock& operator=(const StateLock&) = delete;
};

std::uint64_t unix_timestamp_ms() {
  FILETIME file_time{};
  GetSystemTimePreciseAsFileTime(&file_time);
  ULARGE_INTEGER value{};
  value.LowPart = file_time.dwLowDateTime;
  value.HighPart = file_time.dwHighDateTime;
  constexpr std::uint64_t kWindowsToUnixEpoch100ns = 116444736000000000ull;
  return (value.QuadPart - kWindowsToUnixEpoch100ns) / 10'000ull;
}

void clear_text(char* dst, const std::size_t size) {
  if (dst != nullptr && size != 0) std::memset(dst, 0, size);
}

void store_text(char* dst, const std::size_t size, const char* src) {
  if (dst == nullptr || size == 0) return;
  clear_text(dst, size);
  if (src == nullptr) return;
  const std::size_t count = std::min(size - 1, std::strlen(src));
  std::memcpy(dst, src, count);
}

std::string module_path_utf8() {
  wchar_t wide_path[32768] = {};
  const DWORD length = GetModuleFileNameW(g_module, wide_path, ARRAYSIZE(wide_path));
  if (length == 0 || length >= ARRAYSIZE(wide_path)) return "<unavailable>";
  const int bytes = WideCharToMultiByte(
      CP_UTF8, 0, wide_path, static_cast<int>(length), nullptr, 0, nullptr, nullptr);
  if (bytes <= 0) return "<conversion-failed>";
  std::string path(static_cast<std::size_t>(bytes), '\0');
  WideCharToMultiByte(
      CP_UTF8, 0, wide_path, static_cast<int>(length), path.data(), bytes, nullptr, nullptr);
  return path;
}

void log_message(const char* text) {
  if (g_game_log != nullptr) g_game_log(SCS_LOG_TYPE_message, text);
}

void log_warning(const char* text) {
  if (g_game_log != nullptr) g_game_log(SCS_LOG_TYPE_warning, text);
}

void log_error(const char* text) {
  if (g_game_log != nullptr) g_game_log(SCS_LOG_TYPE_error, text);
}

void log_windows_error(const char* operation) {
  char message[256] = {};
  std::snprintf(message, sizeof(message), "[SimNexus] %s failed: win32=%lu", operation,
                static_cast<unsigned long>(GetLastError()));
  log_error(message);
}

std::uint64_t fnv1a64(const char* text) {
  if (text == nullptr || *text == '\0') return 0;
  std::uint64_t hash = 14695981039346656037ull;
  for (const unsigned char* p = reinterpret_cast<const unsigned char*>(text); *p != 0; ++p) {
    hash ^= static_cast<std::uint64_t>(*p);
    hash *= 1099511628211ull;
  }
  return hash;
}

void clear_job_fields() {
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
  clear_text(g_state.cargo_id, sizeof(g_state.cargo_id));
  clear_text(g_state.source_city_id, sizeof(g_state.source_city_id));
  clear_text(g_state.destination_city_id, sizeof(g_state.destination_city_id));
  clear_text(g_state.source_company_id, sizeof(g_state.source_company_id));
  clear_text(g_state.destination_company_id, sizeof(g_state.destination_company_id));
  clear_text(g_state.job_market, sizeof(g_state.job_market));
  g_last_job_hash = 0;
}

void update_job_id() {
  char fingerprint[768] = {};
  std::snprintf(
      fingerprint, sizeof(fingerprint), "%s|%s|%s|%s|%s|%lld|%u|%.0f|%u",
      g_state.source_city_id[0] ? g_state.source_city_id : g_state.source_city,
      g_state.destination_city_id[0] ? g_state.destination_city_id : g_state.destination_city,
      g_state.source_company_id[0] ? g_state.source_company_id : g_state.source_company,
      g_state.destination_company_id[0] ? g_state.destination_company_id
                                        : g_state.destination_company,
      g_state.cargo_id[0] ? g_state.cargo_id : g_state.cargo,
      static_cast<long long>(g_state.job_income),
      static_cast<unsigned>(g_state.job_delivery_time_min),
      g_state.job_planned_distance_km, static_cast<unsigned>(g_state.job_special));
  const std::uint64_t hash = fnv1a64(fingerprint);
  char id[64] = {};
  std::snprintf(id, sizeof(id), "job-%016llx", static_cast<unsigned long long>(hash));
  store_text(g_state.job_id, sizeof(g_state.job_id), id);
  if (hash == 0 || hash == g_last_job_hash) return;
  g_last_job_hash = hash;
  char message[640] = {};
  std::snprintf(
      message, sizeof(message),
      "[SimNexus] Job detected: id=%s route=%s->%s cargo=%s income=%lld planned_km=%.1f delivery_min=%u market=%s special=%u",
      g_state.job_id, g_state.source_city, g_state.destination_city, g_state.cargo,
      static_cast<long long>(g_state.job_income), g_state.job_planned_distance_km,
      static_cast<unsigned>(g_state.job_delivery_time_min), g_state.job_market,
      static_cast<unsigned>(g_state.job_special));
  log_message(message);
}

void publish_snapshot(const TelemetryData& snapshot) {
  if (g_header == nullptr || g_payload == nullptr) return;
  AcquireSRWLockExclusive(&g_publish_lock);
  InterlockedIncrement64(reinterpret_cast<volatile LONG64*>(&g_header->sequence));
  MemoryBarrier();
  std::memcpy(g_payload, &snapshot, sizeof(snapshot));
  MemoryBarrier();
  InterlockedIncrement64(reinterpret_cast<volatile LONG64*>(&g_header->sequence));
  ReleaseSRWLockExclusive(&g_publish_lock);
}

void publish_current_snapshot() {
  TelemetryData snapshot{};
  AcquireSRWLockShared(&g_state_lock);
  snapshot = g_state;
  ReleaseSRWLockShared(&g_state_lock);
  snapshot.heartbeat_timestamp_ms = unix_timestamp_ms();
  publish_snapshot(snapshot);
}

DWORD WINAPI heartbeat_main(void*) {
  while (WaitForSingleObject(g_heartbeat_stop, 0) == WAIT_TIMEOUT) {
    publish_current_snapshot();
    if (WaitForSingleObject(g_heartbeat_stop, simnexus::kHeartbeatIntervalMs) != WAIT_TIMEOUT) break;
  }
  return 0;
}

bool start_heartbeat() {
  g_heartbeat_stop = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  if (g_heartbeat_stop == nullptr) {
    log_windows_error("CreateEventW(heartbeat)");
    return false;
  }
  g_heartbeat_thread = CreateThread(nullptr, 0, heartbeat_main, nullptr, 0, nullptr);
  if (g_heartbeat_thread == nullptr) {
    log_windows_error("CreateThread(heartbeat)");
    CloseHandle(g_heartbeat_stop);
    g_heartbeat_stop = nullptr;
    return false;
  }
  return true;
}

void stop_heartbeat() {
  if (g_heartbeat_stop != nullptr) SetEvent(g_heartbeat_stop);
  if (g_heartbeat_thread != nullptr) {
    WaitForSingleObject(g_heartbeat_thread, 2'000);
    CloseHandle(g_heartbeat_thread);
    g_heartbeat_thread = nullptr;
  }
  if (g_heartbeat_stop != nullptr) {
    CloseHandle(g_heartbeat_stop);
    g_heartbeat_stop = nullptr;
  }
}

bool initialize_shared_memory(
    const scs_u32_t sdk_version, const scs_sdk_init_params_v100_t& common) {
  const DWORD mapping_size =
      static_cast<DWORD>(sizeof(TelemetryBridgeHeader) + sizeof(TelemetryData));
  SetLastError(ERROR_SUCCESS);
  g_mapping = CreateFileMappingW(
      INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE, 0, mapping_size, simnexus::kSharedMemoryName);
  if (g_mapping == nullptr) {
    log_windows_error("CreateFileMappingW");
    return false;
  }
  const bool mapping_preexisting = GetLastError() == ERROR_ALREADY_EXISTS;
  auto* base = static_cast<std::uint8_t*>(
      MapViewOfFile(g_mapping, FILE_MAP_ALL_ACCESS, 0, 0, mapping_size));
  if (base == nullptr) {
    log_windows_error("MapViewOfFile");
    CloseHandle(g_mapping);
    g_mapping = nullptr;
    return false;
  }

  g_header = reinterpret_cast<TelemetryBridgeHeader*>(base);
  g_payload = reinterpret_cast<TelemetryData*>(base + sizeof(TelemetryBridgeHeader));
  std::memset(base, 0, mapping_size);
  std::memcpy(g_header->magic, simnexus::kBridgeMagic, sizeof(g_header->magic));
  g_header->protocol_version = simnexus::kBridgeProtocolVersion;
  g_header->payload_size = static_cast<std::uint32_t>(sizeof(TelemetryData));
  g_header->sequence = 0;

  const std::string dll_path = module_path_utf8();
  {
    StateLock lock;
    std::memset(&g_state, 0, sizeof(g_state));
    g_state.heartbeat_timestamp_ms = unix_timestamp_ms();
    g_state.payload_revision = simnexus::kPayloadRevision;
    g_state.game_version = common.game_version;
    g_state.telemetry_sdk_version = sdk_version;
    g_state.process_id = GetCurrentProcessId();
    g_state.plugin_initialized = 1;
    g_state.game_paused = 1;
    g_state.mapping_preexisting = mapping_preexisting ? 1 : 0;
    store_text(g_state.build_id, sizeof(g_state.build_id), SIMNEXUS_BUILD_ID);
    store_text(g_state.dll_version, sizeof(g_state.dll_version), SIMNEXUS_DLL_VERSION);
    store_text(g_state.game_id, sizeof(g_state.game_id), common.game_id);
    store_text(g_state.dll_path, sizeof(g_state.dll_path), dll_path.c_str());
  }
  publish_current_snapshot();

  char message[1024] = {};
  std::snprintf(
      message, sizeof(message),
      "[SimNexus DLL] Loaded DLL: %s | Build ID: %s | DLL version: %s | Architecture: %s | Pointer size: %zu | Protocol: %u | Header: %zu | Payload: %zu | Mapping preexisting: %s",
      dll_path.c_str(), SIMNEXUS_BUILD_ID, SIMNEXUS_DLL_VERSION,
#if defined(_M_X64)
      "x64",
#elif defined(_M_IX86)
      "x86",
#else
      "unknown",
#endif
      sizeof(void*), simnexus::kBridgeProtocolVersion, sizeof(TelemetryBridgeHeader),
      sizeof(TelemetryData), mapping_preexisting ? "true" : "false");
  log_message(message);
  std::snprintf(
      message, sizeof(message),
      "[SimNexus DLL] Layout: align=%zu heartbeat=%zu telemetry_timestamp=%zu frame_id=%zu job_active=%zu job_event=%zu job_id=%zu source_city=%zu",
      alignof(TelemetryData), offsetof(TelemetryData, heartbeat_timestamp_ms),
      offsetof(TelemetryData, telemetry_timestamp_ms), offsetof(TelemetryData, frame_id),
      offsetof(TelemetryData, job_active), offsetof(TelemetryData, job_event),
      offsetof(TelemetryData, job_id), offsetof(TelemetryData, source_city));
  log_message(message);
  return true;
}

void shutdown_shared_memory() {
  if (g_header != nullptr) UnmapViewOfFile(g_header);
  g_header = nullptr;
  g_payload = nullptr;
  if (g_mapping != nullptr) CloseHandle(g_mapping);
  g_mapping = nullptr;
}

SCSAPI_VOID telemetry_store_float(
    const scs_string_t, const scs_u32_t, const scs_value_t* const value,
    const scs_context_t context) {
  if (value == nullptr || context == nullptr) return;
  StateLock lock;
  *static_cast<float*>(context) = value->value_float.value;
  g_state.telemetry_callback_seen = 1;
}

SCSAPI_VOID telemetry_store_float_to_double(
    const scs_string_t, const scs_u32_t, const scs_value_t* const value,
    const scs_context_t context) {
  if (value == nullptr || context == nullptr) return;
  StateLock lock;
  *static_cast<double*>(context) = static_cast<double>(value->value_float.value);
  g_state.telemetry_callback_seen = 1;
}

SCSAPI_VOID telemetry_store_s32(
    const scs_string_t, const scs_u32_t, const scs_value_t* const value,
    const scs_context_t context) {
  if (value == nullptr || context == nullptr) return;
  StateLock lock;
  *static_cast<std::int32_t*>(context) = value->value_s32.value;
  g_state.telemetry_callback_seen = 1;
}

SCSAPI_VOID telemetry_store_u32(
    const scs_string_t, const scs_u32_t, const scs_value_t* const value,
    const scs_context_t context) {
  if (value == nullptr || context == nullptr) return;
  StateLock lock;
  *static_cast<std::uint32_t*>(context) = value->value_u32.value;
  g_state.telemetry_callback_seen = 1;
}

SCSAPI_VOID telemetry_store_bool(
    const scs_string_t, const scs_u32_t, const scs_value_t* const value,
    const scs_context_t context) {
  if (value == nullptr || context == nullptr) return;
  StateLock lock;
  *static_cast<std::uint8_t*>(context) = value->value_bool.value ? 1 : 0;
  g_state.telemetry_callback_seen = 1;
}

SCSAPI_VOID telemetry_store_speed(
    const scs_string_t, const scs_u32_t, const scs_value_t* const value,
    const scs_context_t) {
  if (value == nullptr) return;
  StateLock lock;
  g_state.speed_kph = static_cast<double>(value->value_float.value) * 3.6;
  g_state.telemetry_callback_seen = 1;
}

SCSAPI_VOID telemetry_configuration(
    const scs_event_t, const void* const event_info, const scs_context_t) {
  const auto* config = static_cast<const scs_telemetry_configuration_t*>(event_info);
  if (config == nullptr || config->id == nullptr || config->attributes == nullptr) return;
  StateLock lock;
  g_state.telemetry_callback_seen = 1;

  for (const scs_named_value_t* a = config->attributes; a->name != nullptr; ++a) {
    if (std::strcmp(a->name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_fuel_capacity) == 0 &&
        a->value.type == SCS_VALUE_TYPE_float) {
      g_state.fuel_capacity_liters = a->value.value_float.value;
    }
  }
  if (std::strcmp(config->id, SCS_TELEMETRY_CONFIG_job) != 0) return;

  g_state.job_config_seen = 1;
  if (config->attributes->name == nullptr) {
    if (g_state.job_event == 0) clear_job_fields();
    else g_state.job_active = 0;
    log_message("[SimNexus] Job configuration cleared");
    return;
  }

  clear_job_fields();
  g_state.job_active = 1;
  g_state.job_config_seen = 1;
  for (const scs_named_value_t* a = config->attributes; a->name != nullptr; ++a) {
    const char* name = a->name;
    const scs_value_t& value = a->value;
#define COPY_JOB_STRING(attribute_name, field_name)                                      \
    if (std::strcmp(name, attribute_name) == 0 && value.type == SCS_VALUE_TYPE_string) { \
      store_text(g_state.field_name, sizeof(g_state.field_name), value.value_string.value); \
      continue;                                                                          \
    }
    COPY_JOB_STRING(SCS_TELEMETRY_CONFIG_ATTRIBUTE_source_city, source_city)
    COPY_JOB_STRING(SCS_TELEMETRY_CONFIG_ATTRIBUTE_destination_city, destination_city)
    COPY_JOB_STRING(SCS_TELEMETRY_CONFIG_ATTRIBUTE_source_company, source_company)
    COPY_JOB_STRING(SCS_TELEMETRY_CONFIG_ATTRIBUTE_destination_company, destination_company)
    COPY_JOB_STRING(SCS_TELEMETRY_CONFIG_ATTRIBUTE_cargo, cargo)
    COPY_JOB_STRING(SCS_TELEMETRY_CONFIG_ATTRIBUTE_cargo_id, cargo_id)
    COPY_JOB_STRING(SCS_TELEMETRY_CONFIG_ATTRIBUTE_source_city_id, source_city_id)
    COPY_JOB_STRING(SCS_TELEMETRY_CONFIG_ATTRIBUTE_destination_city_id, destination_city_id)
    COPY_JOB_STRING(SCS_TELEMETRY_CONFIG_ATTRIBUTE_source_company_id, source_company_id)
    COPY_JOB_STRING(SCS_TELEMETRY_CONFIG_ATTRIBUTE_destination_company_id, destination_company_id)
    COPY_JOB_STRING(SCS_TELEMETRY_CONFIG_ATTRIBUTE_job_market, job_market)
#undef COPY_JOB_STRING
    if (std::strcmp(name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_income) == 0) {
      if (value.type == SCS_VALUE_TYPE_u64)
        g_state.job_income = static_cast<std::int64_t>(value.value_u64.value);
      else if (value.type == SCS_VALUE_TYPE_s64)
        g_state.job_income = value.value_s64.value;
      else if (value.type == SCS_VALUE_TYPE_u32)
        g_state.job_income = value.value_u32.value;
      else if (value.type == SCS_VALUE_TYPE_s32)
        g_state.job_income = value.value_s32.value;
      continue;
    }
    if (std::strcmp(name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_delivery_time) == 0) {
      if (value.type == SCS_VALUE_TYPE_u32) g_state.job_delivery_time_min = value.value_u32.value;
      else if (value.type == SCS_VALUE_TYPE_s32)
        g_state.job_delivery_time_min = static_cast<std::uint32_t>(value.value_s32.value);
      continue;
    }
    if (std::strcmp(name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_planned_distance_km) == 0) {
      if (value.type == SCS_VALUE_TYPE_u32) g_state.job_planned_distance_km = value.value_u32.value;
      else if (value.type == SCS_VALUE_TYPE_s32)
        g_state.job_planned_distance_km = value.value_s32.value;
      else if (value.type == SCS_VALUE_TYPE_float)
        g_state.job_planned_distance_km = value.value_float.value;
      else if (value.type == SCS_VALUE_TYPE_double)
        g_state.job_planned_distance_km = value.value_double.value;
      continue;
    }
    if (std::strcmp(name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_special_job) == 0 &&
        value.type == SCS_VALUE_TYPE_bool) {
      g_state.job_special = value.value_bool.value ? 1 : 0;
      continue;
    }
    if (std::strcmp(name, SCS_TELEMETRY_CONFIG_ATTRIBUTE_is_cargo_loaded) == 0 &&
        value.type == SCS_VALUE_TYPE_bool) {
      g_state.job_cargo_loaded = value.value_bool.value ? 1 : 0;
    }
  }
  update_job_id();
}

SCSAPI_VOID telemetry_game_started(const scs_event_t, const void* const, const scs_context_t) {
  StateLock lock;
  g_state.game_paused = 0;
  g_state.telemetry_callback_seen = 1;
}

SCSAPI_VOID telemetry_game_paused(const scs_event_t, const void* const, const scs_context_t) {
  StateLock lock;
  g_state.game_paused = 1;
  g_state.telemetry_callback_seen = 1;
}

SCSAPI_VOID telemetry_frame_end(const scs_event_t, const void* const, const scs_context_t) {
  StateLock lock;
  ++g_state.frame_id;
  g_state.telemetry_timestamp_ms = unix_timestamp_ms();
  g_state.telemetry_active = 1;
  g_state.telemetry_callback_seen = 1;
}

SCSAPI_VOID telemetry_gameplay_event(
    const scs_event_t, const void* const event_info, const scs_context_t) {
  const auto* gameplay = static_cast<const scs_telemetry_gameplay_event_t*>(event_info);
  if (gameplay == nullptr || gameplay->id == nullptr) return;
  std::uint8_t job_event = 0;
  if (std::strcmp(gameplay->id, SCS_TELEMETRY_GAMEPLAY_EVENT_job_delivered) == 0) job_event = 1;
  else if (std::strcmp(gameplay->id, SCS_TELEMETRY_GAMEPLAY_EVENT_job_cancelled) == 0) job_event = 2;
  else return;
  {
    StateLock lock;
    g_state.telemetry_callback_seen = 1;
    g_state.job_event = job_event;
    ++g_state.job_event_sequence;
    if (gameplay->attributes != nullptr) {
      for (const scs_named_value_t* a = gameplay->attributes; a->name != nullptr; ++a) {
        if (std::strcmp(a->name, SCS_TELEMETRY_GAMEPLAY_EVENT_ATTRIBUTE_cargo_damage) != 0)
          continue;
        if (a->value.type == SCS_VALUE_TYPE_float)
          g_state.job_cargo_damage = a->value.value_float.value;
        else if (a->value.type == SCS_VALUE_TYPE_double)
          g_state.job_cargo_damage = a->value.value_double.value;
      }
    }
  }
  log_message(job_event == 1 ? "[SimNexus] Job delivered event received"
                             : "[SimNexus] Job cancelled event received");
  publish_current_snapshot();
}

const char* value_type_name(const scs_value_type_t type) {
  switch (type) {
    case SCS_VALUE_TYPE_bool: return "bool";
    case SCS_VALUE_TYPE_s32: return "s32";
    case SCS_VALUE_TYPE_u32: return "u32";
    case SCS_VALUE_TYPE_float: return "float";
    case SCS_VALUE_TYPE_double: return "double";
    default: return "other";
  }
}

const char* event_name(const scs_event_t event) {
  switch (event) {
    case SCS_TELEMETRY_EVENT_started: return "started";
    case SCS_TELEMETRY_EVENT_frame_start: return "frame_start";
    case SCS_TELEMETRY_EVENT_frame_end: return "frame_end";
    case SCS_TELEMETRY_EVENT_paused: return "paused";
    case SCS_TELEMETRY_EVENT_configuration: return "configuration";
    case SCS_TELEMETRY_EVENT_gameplay: return "gameplay";
    default: return "unknown";
  }
}

bool register_event(
    const scs_telemetry_init_params_v100_t* params, const scs_event_t event,
    const scs_telemetry_event_callback_t callback, const bool required) {
  const scs_result_t result = params->register_for_event(event, callback, nullptr);
  char message[256] = {};
  std::snprintf(message, sizeof(message),
                "[SimNexus] Event registration: name=%s id=%u result=%d required=%s",
                event_name(event), static_cast<unsigned>(event),
                static_cast<int>(result), required ? "true" : "false");
  if (result == SCS_RESULT_ok) log_message(message);
  else if (required) log_error(message);
  else log_warning(message);
  return result == SCS_RESULT_ok || !required;
}

void register_channel(
    const scs_telemetry_init_params_v100_t* params, const scs_string_t name,
    const scs_value_type_t type, const scs_telemetry_channel_callback_t callback,
    const scs_context_t context) {
  const scs_result_t result = params->register_for_channel(
      name, SCS_U32_NIL, type, SCS_TELEMETRY_CHANNEL_FLAG_none, callback, context);
  char message[256] = {};
  std::snprintf(message, sizeof(message),
                "[SimNexus] Channel registration: name=%s type=%s result=%d", name,
                value_type_name(type), static_cast<int>(result));
  if (result == SCS_RESULT_ok) log_message(message);
  else log_warning(message);
}

bool register_telemetry_events(
    const scs_u32_t version, const scs_telemetry_init_params_v100_t* params) {
  bool ok = true;
  ok &= register_event(params, SCS_TELEMETRY_EVENT_started, telemetry_game_started, true);
  ok &= register_event(params, SCS_TELEMETRY_EVENT_paused, telemetry_game_paused, true);
  ok &= register_event(params, SCS_TELEMETRY_EVENT_frame_end, telemetry_frame_end, true);
  ok &= register_event(params, SCS_TELEMETRY_EVENT_configuration, telemetry_configuration, true);
  if (version >= SCS_TELEMETRY_VERSION_1_01)
    register_event(params, SCS_TELEMETRY_EVENT_gameplay, telemetry_gameplay_event, false);
  return ok;
}

void register_telemetry_channels(const scs_telemetry_init_params_v100_t* params) {
  register_channel(params, SCS_TELEMETRY_TRUCK_CHANNEL_speed, SCS_VALUE_TYPE_float,
                   telemetry_store_speed, nullptr);
  register_channel(params, SCS_TELEMETRY_TRUCK_CHANNEL_engine_rpm, SCS_VALUE_TYPE_float,
                   telemetry_store_float_to_double, &g_state.engine_rpm);
  register_channel(params, SCS_TELEMETRY_TRUCK_CHANNEL_engine_gear, SCS_VALUE_TYPE_s32,
                   telemetry_store_s32, &g_state.gear);
  register_channel(params, SCS_TELEMETRY_TRUCK_CHANNEL_engine_enabled, SCS_VALUE_TYPE_bool,
                   telemetry_store_bool, &g_state.engine_enabled);
  register_channel(params, SCS_TELEMETRY_TRUCK_CHANNEL_fuel, SCS_VALUE_TYPE_float,
                   telemetry_store_float, &g_state.fuel_liters);
  register_channel(params, SCS_TELEMETRY_TRUCK_CHANNEL_odometer, SCS_VALUE_TYPE_float,
                   telemetry_store_float_to_double, &g_state.odometer_km);
  register_channel(params, SCS_TELEMETRY_CHANNEL_local_scale, SCS_VALUE_TYPE_float,
                   telemetry_store_float, &g_state.map_scale);
  register_channel(params, SCS_TELEMETRY_CHANNEL_game_time, SCS_VALUE_TYPE_u32,
                   telemetry_store_u32, &g_state.game_time_min);
  register_channel(params, SCS_TELEMETRY_JOB_CHANNEL_cargo_damage, SCS_VALUE_TYPE_float,
                   telemetry_store_float_to_double, &g_state.job_cargo_damage);
}

} // namespace

BOOL APIENTRY DllMain(HMODULE module, DWORD reason, LPVOID) {
  if (reason == DLL_PROCESS_ATTACH) {
    g_module = module;
    DisableThreadLibraryCalls(module);
  }
  return TRUE;
}

SCSSDK_HEADER

__declspec(dllexport) SCSAPI_RESULT scs_telemetry_init(
    const scs_u32_t version, const scs_telemetry_init_params_t* const params) {
  if (params == nullptr) return SCS_RESULT_invalid_parameter;
  if (SCS_GET_MAJOR_VERSION(version) != 1) return SCS_RESULT_unsupported;

  const auto* version_params = static_cast<const scs_telemetry_init_params_v100_t*>(params);
  g_game_log = version_params->common.log;
  log_message("[SimNexus] DLL_PROCESS_ATTACH observed");
  log_message("[SimNexus] scs_telemetry_init entered");

  char message[512] = {};
  std::snprintf(
      message, sizeof(message),
      "[SimNexus] SDK received: api=%u.%u game_id=%s game_version=%u.%u",
      SCS_GET_MAJOR_VERSION(version), SCS_GET_MINOR_VERSION(version),
      version_params->common.game_id != nullptr ? version_params->common.game_id : "<null>",
      SCS_GET_MAJOR_VERSION(version_params->common.game_version),
      SCS_GET_MINOR_VERSION(version_params->common.game_version));
  log_message(message);
  if (SCS_GET_MAJOR_VERSION(version_params->common.game_version) == 1 &&
      SCS_GET_MINOR_VERSION(version_params->common.game_version) > 10) {
    log_warning("[SimNexus] Newer compatible game telemetry version detected; continuing");
  }

  if (!initialize_shared_memory(version, version_params->common))
    return SCS_RESULT_generic_error;
  if (!register_telemetry_events(version, version_params)) {
    shutdown_shared_memory();
    log_error("[SimNexus] Required callback registration failed");
    return SCS_RESULT_generic_error;
  }
  register_telemetry_channels(version_params);
  {
    StateLock lock;
    g_state.sdk_connected = 1;
  }
  publish_current_snapshot();
  if (!start_heartbeat()) {
    shutdown_shared_memory();
    return SCS_RESULT_generic_error;
  }
  log_message("[SimNexus] callbacks registered");
  log_message("[SimNexus] channels registered");
  log_message("[SimNexus] initialization completed");
  return SCS_RESULT_ok;
}

__declspec(dllexport) SCSAPI_VOID scs_telemetry_shutdown(void) {
  log_message("[SimNexus] scs_telemetry_shutdown entered");
  stop_heartbeat();
  {
    StateLock lock;
    g_state.sdk_connected = 0;
    g_state.telemetry_active = 0;
  }
  publish_current_snapshot();
  shutdown_shared_memory();
  log_message("[SimNexus] telemetry bridge shut down");
  g_game_log = nullptr;
}

SCSSDK_FOOTER
