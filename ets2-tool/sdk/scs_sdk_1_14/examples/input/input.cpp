/**
 * @brief Simple input device
 *
 * Generates device with a few axes and buttons which change with time
 */

// Windows stuff.

#ifdef _WIN32
#  define WINVER 0x0500
#  define _WIN32_WINNT 0x0500
#  include <windows.h>
#endif

#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#include <stdarg.h>
#include <string.h>
#include <time.h>

// SDK

#include "scssdk_input.h"
#include "eurotrucks2/scssdk_eut2.h"
#include "eurotrucks2/scssdk_input_eut2.h"
#include "amtrucks/scssdk_ats.h"
#include "amtrucks/scssdk_input_ats.h"

#define UNUSED(x)

/**
 * @brief Logging support.
 */
FILE *log_file = NULL;

const unsigned AXIS_COUNT = 2;
const unsigned BUTTON_COUNT = 3;

// Simulation of the values.

struct device_data_t
{
	// Current state of the dummy inputs.

	float axes[AXIS_COUNT];
	unsigned button_state;

	// Last time when we simulated movement.

	time_t last_movement_time;

	// Index of next input we will report.

	unsigned next_reported_input;
};

device_data_t dummy_device;

// Management of the log file.

bool init_log(void)
{
	if (log_file) {
		return true;
	}
	log_file = fopen("input.log", "wt");
	if (! log_file) {
		return false;
	}
	fprintf(log_file, "Log opened\n");
	return true;
}

void finish_log(void)
{
	if (! log_file) {
		return;
	}
	fprintf(log_file, "Log ended\n");
	fclose(log_file);
	log_file = NULL;
}

void log_print(const char *const text, ...)
{
	if (! log_file) {
		return;
	}
	va_list args;
	va_start(args, text);
	vfprintf(log_file, text, args);
	va_end(args);
}

void log_line(const char *const text, ...)
{
	if (! log_file) {
		return;
	}
	va_list args;
	va_start(args, text);
	vfprintf(log_file, text, args);
	fprintf(log_file, "\n");
	va_end(args);
}

void simulate_device(device_data_t &device)
{
	// Move once per second.

	const time_t now = time(NULL);
	if (device.last_movement_time == now) {
		return;
	}
	device.last_movement_time = now;

	// Move each axis to maximum before moving the next
	// one reseting everything to zero if all of them
	// reach maximum.

	if (device.axes[AXIS_COUNT - 1] >= 1.0f) {
		memset(device.axes, 0, sizeof(device.axes));
	}

	for (unsigned i = 0; i < AXIS_COUNT; ++i) {
		if (device.axes[i] < 1.0f) {
			device.axes[i] += 0.125f;
			break;
		}
	}

	device.button_state = (device.button_state + 1) % (BUTTON_COUNT * 2);
}

SCSAPI_VOID input_active_callback(const scs_u8_t active, const scs_context_t UNUSED(context))
{
	// This callback is optional. A real implementation could do some cleanup
	// on deactivation or can leave it to event callback with
	// SCS_INPUT_EVENT_CALLBACK_FLAG_first_after_activation flag.

	if (active > 0) {
		log_line("Device activated");
	}
	else {
		log_line("Device deactivated");
	}
}

SCSAPI_RESULT input_event_callback(scs_input_event_t *const event_info, const scs_u32_t flags, const scs_context_t context)
{
	device_data_t &device = *static_cast<device_data_t *>(context);

	if (flags & SCS_INPUT_EVENT_CALLBACK_FLAG_first_after_activation) {
		log_line("First call after activation");
	}

	// Simulates retrieval of current data from some external device at start
	// of the frame.

	if (flags & SCS_INPUT_EVENT_CALLBACK_FLAG_first_in_frame) {
		simulate_device(device);
		device.next_reported_input = 0;
	}

	// Did we process all events for this frame?

	if (device.next_reported_input >= (AXIS_COUNT + BUTTON_COUNT)) {
		return SCS_RESULT_not_found;
	}

	// Generate events for individual inputs. If the number of inputs is small,
	// it is fine to generate events even if there was no change.

	event_info->input_index = device.next_reported_input;
	if (device.next_reported_input < AXIS_COUNT) {
		event_info->value_float.value = device.axes[device.next_reported_input];
	}
	else {
		event_info->value_bool.value = (((device.next_reported_input - AXIS_COUNT) * 2) == device.button_state) ? 1 : 0;
	}
	++device.next_reported_input;
	return SCS_RESULT_ok;
}

/**
 * @brief Input API initialization function.
 *
 * See scssdk_input.h
 */
SCSAPI_RESULT scs_input_init(const scs_u32_t version, const scs_input_init_params_t *const params)
{
	// We currently support only one version.

	if (version != SCS_INPUT_VERSION_1_00) {
		return SCS_RESULT_unsupported;
	}

	const scs_input_init_params_v100_t *const version_params = static_cast<const scs_input_init_params_v100_t *>(params);
	if (! init_log()) {
		version_params->common.log(SCS_LOG_TYPE_error, "Unable to initialize the log file");
		return SCS_RESULT_generic_error;
	}

	// Check application version. Note that this example uses fairly basic features which are likely to be supported
	// by any future SCS trucking game however more advanced applications might want to at least warn the user if there
	// is game or version they do not support.

	log_line("Game '%s' %u.%u", version_params->common.game_id, SCS_GET_MAJOR_VERSION(version_params->common.game_version), SCS_GET_MINOR_VERSION(version_params->common.game_version));

	if (strcmp(version_params->common.game_id, SCS_GAME_ID_EUT2) == 0) {

		// Below the minimum version there might be some missing features (only minor change) or
		// incompatible values (major change).

		const scs_u32_t MINIMAL_VERSION = SCS_INPUT_EUT2_GAME_VERSION_1_00;
		if (version_params->common.game_version < MINIMAL_VERSION) {
			log_line("WARNING: Too old version of the game, some features might behave incorrectly");
		}

		// Future versions are fine as long the major version is not changed.

		const scs_u32_t IMPLEMENTED_VERSION = SCS_INPUT_EUT2_GAME_VERSION_CURRENT;
		if (SCS_GET_MAJOR_VERSION(version_params->common.game_version) > SCS_GET_MAJOR_VERSION(IMPLEMENTED_VERSION)) {
			log_line("WARNING: Too new major version of the game, some features might behave incorrectly");
		}
	}
	else if (strcmp(version_params->common.game_id, SCS_GAME_ID_ATS) == 0) {

		// Below the minimum version there might be some missing features (only minor change) or
		// incompatible values (major change).

		const scs_u32_t MINIMAL_VERSION = SCS_INPUT_ATS_GAME_VERSION_1_00;
		if (version_params->common.game_version < MINIMAL_VERSION) {
			log_line("WARNING: Too old version of the game, some features might behave incorrectly");
		}

		// Future versions are fine as long the major version is not changed.

		const scs_u32_t IMPLEMENTED_VERSION = SCS_INPUT_ATS_GAME_VERSION_CURRENT;
		if (SCS_GET_MAJOR_VERSION(version_params->common.game_version) > SCS_GET_MAJOR_VERSION(IMPLEMENTED_VERSION)) {
			log_line("WARNING: Too new major version of the game, some features might behave incorrectly");
		}
	}
	else {
		log_line("WARNING: Unsupported game, some features or values might behave incorrectly");
	}

	// Setup the device information.

	const scs_input_device_input_t inputs[] = {
		{"a1", "Analog axis 1", SCS_VALUE_TYPE_float},
		{"a2", "Analog axis 2", SCS_VALUE_TYPE_float},
		{"b1", "Button 1", SCS_VALUE_TYPE_bool},
		{"b2", "Button 2", SCS_VALUE_TYPE_bool},
		{"b3", "Button 3", SCS_VALUE_TYPE_bool},
	};
	assert(AXIS_COUNT == 2);
	assert(BUTTON_COUNT == 3);

	scs_input_device_t device_info;
	memset(&device_info, 0, sizeof(device_info));
	device_info.name = "example";
	device_info.display_name = "Example SDK Device";
	device_info.type = SCS_INPUT_DEVICE_TYPE_generic;
	device_info.input_count = sizeof(inputs) / sizeof(inputs[0]);
	device_info.inputs = inputs;

	device_info.input_active_callback = input_active_callback;
	device_info.input_event_callback = input_event_callback;
	device_info.callback_context = &dummy_device;

	memset(&dummy_device, 0, sizeof(dummy_device));
	dummy_device.last_movement_time = time(NULL);

	if (version_params->register_device(&device_info) != SCS_RESULT_ok) {

		// Registrations created by unsuccessfull initialization are
		// cleared automatically so we can simply exit.

		version_params->common.log(SCS_LOG_TYPE_error, "Unable to register device");
		return SCS_RESULT_generic_error;
	}

	return SCS_RESULT_ok;
}

/**
 * @brief Input API deinitialization function.
 *
 * See scssdk_input.h
 */
SCSAPI_VOID scs_input_shutdown(void)
{
	// Any cleanup needed. The registrations will be removed automatically.

	finish_log();
}

// Cleanup

#ifdef _WIN32
BOOL APIENTRY DllMain(
	HMODULE module,
	DWORD  reason_for_call,
	LPVOID reseved
)
{
	if (reason_for_call == DLL_PROCESS_DETACH) {
		finish_log();
	}
	return TRUE;
}
#endif

#ifdef __linux__
void __attribute__ ((destructor)) unload(void)
{
	finish_log();
}
#endif
