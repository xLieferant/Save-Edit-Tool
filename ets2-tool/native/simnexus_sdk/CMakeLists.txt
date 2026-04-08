cmake_minimum_required(VERSION 3.20)
project(simnexus_sdk LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

if(NOT DEFINED SCS_SDK_DIR)
  set(SCS_SDK_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../sdk/scs_sdk_1_14/include")
endif()

if(NOT EXISTS "${SCS_SDK_DIR}/scssdk.h")
  message(FATAL_ERROR "SCS SDK headers not found. Expected scssdk.h inside ${SCS_SDK_DIR}.")
endif()

add_library(simnexus_sdk SHARED
  src/simnexus_sdk.cpp
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
)

target_link_libraries(simnexus_sdk
  PRIVATE
    user32
)

set_target_properties(simnexus_sdk PROPERTIES
  OUTPUT_NAME "simnexus_sdk"
)
