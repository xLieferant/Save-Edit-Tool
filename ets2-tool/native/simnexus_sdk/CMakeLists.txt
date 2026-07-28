cmake_minimum_required(VERSION 3.20)
project(simnexus_sdk LANGUAGES CXX RC)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

if(NOT DEFINED SCS_SDK_DIR)
  set(SCS_SDK_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../sdk/scs_sdk_1_14/include")
endif()

if(NOT EXISTS "${SCS_SDK_DIR}/scssdk.h")
  message(FATAL_ERROR "SCS SDK headers not found. Expected scssdk.h inside ${SCS_SDK_DIR}.")
endif()

string(TIMESTAMP SIMNEXUS_BUILD_TIMESTAMP "%Y%m%dT%H%M%SZ" UTC)
execute_process(
  COMMAND git rev-parse --short=12 HEAD
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
  RESULT_VARIABLE SIMNEXUS_GIT_RESULT
  OUTPUT_VARIABLE SIMNEXUS_GIT_SHA
  OUTPUT_STRIP_TRAILING_WHITESPACE
  ERROR_QUIET
)
if(NOT SIMNEXUS_GIT_RESULT EQUAL 0 OR SIMNEXUS_GIT_SHA STREQUAL "")
  set(SIMNEXUS_GIT_SHA "nogit")
endif()
set(SIMNEXUS_BUILD_ID "${SIMNEXUS_BUILD_TIMESTAMP}-${SIMNEXUS_GIT_SHA}")
message(STATUS "SimNexus build ID: ${SIMNEXUS_BUILD_ID}")

add_library(simnexus_sdk SHARED
  src/simnexus_sdk.cpp
  src/simnexus_sdk.rc
)

target_include_directories(simnexus_sdk
  PRIVATE
    ${SCS_SDK_DIR}
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)

target_compile_definitions(simnexus_sdk
  PRIVATE
    WIN32_LEAN_AND_MEAN
    NOMINMAX
    SIMNEXUS_BUILD_ID="${SIMNEXUS_BUILD_ID}"
    SIMNEXUS_DLL_VERSION="3.0.0"
)

if(MSVC)
  target_compile_options(simnexus_sdk PRIVATE /W4 /permissive- /utf-8)
endif()

target_link_libraries(simnexus_sdk
  PRIVATE
    user32
)

set_target_properties(simnexus_sdk PROPERTIES
  OUTPUT_NAME "simnexus_sdk"
)

add_executable(simnexus_bridge_selftest
  tests/simnexus_bridge_selftest.cpp
)
target_include_directories(simnexus_bridge_selftest PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/include)
target_compile_definitions(simnexus_bridge_selftest PRIVATE WIN32_LEAN_AND_MEAN NOMINMAX)
if(MSVC)
  target_compile_options(simnexus_bridge_selftest PRIVATE /W4 /permissive- /utf-8)
endif()
