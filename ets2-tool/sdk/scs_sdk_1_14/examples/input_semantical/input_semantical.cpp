/**
 * @brief Simple semantical input device
 *
 * Generates a simple device which toggles lights based on time
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

SCSAPI_RESULT input_event_callback(scs_input_event_t *const event_info, const scs_u32_t flags, const scs_context_t UNUSED(context))
{
	if ((flags & SCS_INPUT_EVENT_CALLBACK_FLAG_first_in_frame) == 0) {
		return SCS_RESULT_not_found;
	}

	event_info->input_index = 0;
	event_info->value_bool.value = (time(NULL) & 1);
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

	// Setup the device information. The name of the input matches the name of the
	// mix as seen in controls.sii. Note that only some inputs are supported this way.
	// See documentation of SCS_INPUT_DEVICE_TYPE_semantical

	const scs_input_device_input_t input = {"light", "Lights", SCS_VALUE_TYPE_bool};

	scs_input_device_t device_info;
	memset(&device_info, 0, sizeof(device_info));
	device_info.name = "example_semantical";
	device_info.display_name = "Example Semantical SDK Device";
	device_info.type = SCS_INPUT_DEVICE_TYPE_semantical;
	device_info.input_count = 1;
	device_info.inputs = &input;
	device_info.input_event_callback = input_event_callback;
	device_info.callback_context = NULL;

	if (version_params->register_device(&device_info) != SCS_RESULT_ok) {
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
}

// Cleanup

#ifdef _WIN32
BOOL APIENTRY DllMain(
	HMODULE module,
	DWORD  reason_for_call,
	LPVOID reseved
)
{
	return TRUE;
}
#endif
