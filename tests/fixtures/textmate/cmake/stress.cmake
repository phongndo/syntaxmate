cmake_minimum_required(VERSION 3.21)
cmake_policy(VERSION 3.21)
project(AtlasFixture VERSION 4.2.1 DESCRIPTION "Café 日本語 🚀 𝌆" LANGUAGES C CXX)

#[=[
This bracket comment spans lines and contains "quotes", ${NOT_EXPANDED},
a backslash escape \n, café, 日本語, 🚀, and the astral symbol 𝌆.
]=]
# Ordinary comments remain single-line tokens.
set(BUILD_SHARED_LIBS OFF CACHE BOOL "Prefer static libraries")
option(ATLAS_BUILD_TESTS "Build tests" ON)
option(ATLAS_WITH_TOOLS "Build command-line tools" TRUE)
mark_as_advanced(BUILD_SHARED_LIBS)
set(CMAKE_BUILD_TYPE Debug CACHE STRING "Build configuration")
set_property(CACHE CMAKE_BUILD_TYPE PROPERTY STRINGS Debug Release RelWithDebInfo)
set(CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")
set(CMAKE_PREFIX_PATH "$ENV{ATLAS_PREFIX};${CMAKE_PREFIX_PATH}")
set(CMAKE_INSTALL_PREFIX "${CMAKE_CURRENT_BINARY_DIR}/stage" CACHE PATH "Install root")
set(CMAKE_COLOR_MAKEFILE ON)
set(CMAKE_VERBOSE_MAKEFILE FALSE)
set(CMAKE_WARN_ON_ABSOLUTE_INSTALL_DESTINATION ON)
message(STATUS "Generator: ${CMAKE_GENERATOR}")
message(STATUS "Project: ${PROJECT_NAME} from ${PROJECT_SOURCE_DIR}")
message(STATUS "Binary tree: ${PROJECT_BINARY_DIR}")
message(STATUS "Host: ${CMAKE_HOST_SYSTEM_NAME} ${CMAKE_HOST_SYSTEM_PROCESSOR}")
message(STATUS "Tool: ${CMAKE_COMMAND}; environment: $ENV{PATH}")
site_name(BUILD_SITE)

if(APPLE OR CMAKE_HOST_APPLE)
  set(ATLAS_PLATFORM macOS)
elseif(WIN32 AND MSVC)
  set(ATLAS_PLATFORM windows)
elseif(CYGWIN)
  set(ATLAS_PLATFORM cygwin)
elseif(UNIX AND NOT BORLAND)
  set(ATLAS_PLATFORM unix)
else()
  set(ATLAS_PLATFORM unknown)
endif()
if(CMAKE_VERSION VERSION_GREATER_EQUAL "3.21" AND
   CMAKE_CURRENT_SOURCE_DIR STREQUAL PROJECT_SOURCE_DIR)
  message(STATUS "Top-level modern build")
endif()
if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/include" AND
   IS_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/include")
  set(HAVE_PUBLIC_HEADERS TRUE)
endif()
if(DEFINED ATLAS_PLATFORM AND NOT ATLAS_PLATFORM MATCHES "unknown")
  set(PLATFORM_IS_KNOWN ON)
endif()
set(ATLAS_SOURCES
  src/atlas.cpp
  src/codec.cpp
  src/detail/reader.cpp
)
set(ATLAS_HEADERS
  include/atlas/atlas.hpp
  include/atlas/codec.hpp
)
list(APPEND ATLAS_SOURCES src/unicode.cpp)
list(REMOVE_DUPLICATES ATLAS_SOURCES)
list(LENGTH ATLAS_SOURCES ATLAS_SOURCE_COUNT)
set(ATLAS_README [==[
# Atlas
Literal bracket text preserves ${CMAKE_CURRENT_LIST_DIR} and $ENV{HOME}.
It also preserves semicolons; escaped-looking text \t; and café 日本語 🚀 𝌆.
]==])
set(ESCAPED_QUOTES \"quoted unquoted argument\")
string(REPLACE "café" "cafe" ASCII_README "${ATLAS_README}")
string(REGEX MATCH "Atlas" README_MATCH "${ATLAS_README}")
string(TOUPPER "${ATLAS_PLATFORM}" PLATFORM_LABEL)
math(EXPR NEXT_SOURCE_COUNT "${ATLAS_SOURCE_COUNT} + 1")
separate_arguments(EXTRA_FLAGS UNIX_COMMAND "$ENV{ATLAS_FLAGS}")

function(atlas_collect output)
  set(result "")
  foreach(item IN LISTS ARGN)
    if(item MATCHES "\\.(cpp|cxx)$")
      list(APPEND result "${item}")
    else()
      continue()
    endif()
  endforeach()
  set(${output} "${result}" PARENT_SCOPE)
endfunction()
macro(atlas_note text)
  message(STATUS "atlas: ${text}")
endmacro()

atlas_collect(COMPILED_SOURCES ${ATLAS_SOURCES} ${ATLAS_HEADERS})
atlas_note("collecting ${ATLAS_SOURCE_COUNT} source files")
foreach(config Debug Release RelWithDebInfo)
  string(TOUPPER "${config}" config_upper)
  set("ATLAS_POSTFIX_${config_upper}" "-${config}")
endforeach()

set(counter 0)
while(counter LESS 2)
  math(EXPR counter "${counter} + 1")
  if(counter EQUAL 1)
    message(STATUS "First configuration pass")
  else()
    break()
  endif()
endwhile()
find_package(Threads REQUIRED)
find_program(CLANG_FORMAT NAMES clang-format clang-format-18 PATHS "$ENV{HOME}/bin")
find_library(MATH_LIBRARY NAMES m PATHS /usr/lib /usr/local/lib)
find_path(UUID_INCLUDE_DIR NAMES uuid/uuid.h PATHS /usr/include)
find_file(ATLAS_LICENSE NAMES LICENSE LICENSE.txt PATHS "${CMAKE_CURRENT_SOURCE_DIR}")
include(GNUInstallDirs)
include_directories("${CMAKE_CURRENT_SOURCE_DIR}/include")
link_directories("${CMAKE_CURRENT_BINARY_DIR}/lib")

add_library(atlas ${COMPILED_SOURCES} ${ATLAS_HEADERS})
add_library(Atlas::atlas ALIAS atlas)
target_compile_features(atlas PUBLIC cxx_std_20)
target_compile_definitions(atlas
  PRIVATE ATLAS_BUILDING_LIBRARY
  PUBLIC ATLAS_VERSION="${PROJECT_VERSION}"
  INTERFACE ATLAS_CONSUMER=1
)
target_compile_options(atlas PRIVATE -Wall -Wextra)
target_include_directories(atlas
  PUBLIC
    "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>"
    "$<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>"
  PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/src"
)
target_link_libraries(atlas PUBLIC Threads::Threads PRIVATE ${MATH_LIBRARY})
target_sources(atlas PRIVATE src/platform/${ATLAS_PLATFORM}.cpp)

set_target_properties(atlas PROPERTIES
  OUTPUT_NAME atlas_core
  VERSION ${PROJECT_VERSION}
  SOVERSION ${PROJECT_VERSION_MAJOR}
  POSITION_INDEPENDENT_CODE ON
  CXX_VISIBILITY_PRESET hidden
  ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/lib"
  LIBRARY_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/lib"
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/bin"
  INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/include"
)
set_source_files_properties(src/unicode.cpp PROPERTIES
  LANGUAGE CXX
  GENERATED FALSE
  HEADER_FILE_ONLY OFF
  COMPILE_DEFINITIONS "UNICODE_LABEL=日本語"
)
set_directory_properties(PROPERTIES
  ADDITIONAL_MAKE_CLEAN_FILES "${CMAKE_CURRENT_BINARY_DIR}/generated"
  INCLUDE_REGULAR_EXPRESSION ".*\\.(h|hpp)$"
)
set_property(GLOBAL PROPERTY USE_FOLDERS ON)
set_property(GLOBAL PROPERTY RULE_MESSAGES TRUE)
get_target_property(ATLAS_OUTPUT atlas OUTPUT_NAME)
get_source_file_property(UNICODE_LANGUAGE src/unicode.cpp LANGUAGE)
get_directory_property(DIRECTORY_DEFINITIONS COMPILE_DEFINITIONS)
get_cmake_property(ALL_VARIABLES VARIABLES)
get_property(FOLDER_MODE GLOBAL PROPERTY USE_FOLDERS)

add_executable(atlas_cli tools/main.cpp)
target_link_libraries(atlas_cli PRIVATE Atlas::atlas)
set_target_properties(atlas_cli PROPERTIES
  WIN32_EXECUTABLE OFF
  RUNTIME_OUTPUT_NAME atlas
  DEBUG_POSTFIX -d
  FOLDER Tools
)
add_dependencies(atlas_cli atlas)

file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/generated")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/generated/version.txt" "${PROJECT_VERSION}\n")
configure_file(cmake/config.hpp.in generated/atlas/config.hpp @ONLY)
add_custom_command(
  OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/generated/table.cpp"
  COMMAND ${CMAKE_COMMAND} -E echo "café 日本語 🚀 𝌆"
  COMMAND ${CMAKE_COMMAND} -E touch "${CMAKE_CURRENT_BINARY_DIR}/generated/table.cpp"
  DEPENDS cmake/table.txt
  COMMENT "Generating Unicode lookup table"
  VERBATIM
)
add_custom_target(atlas_generated DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/generated/table.cpp")
add_custom_target(format
  COMMAND ${CLANG_FORMAT} -i ${ATLAS_SOURCES}
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
  COMMENT "Formatting Atlas sources"
)
enable_testing()
add_test(NAME atlas.unit COMMAND atlas_cli --self-test)
set_tests_properties(atlas.unit PROPERTIES
  TIMEOUT 30
  WILL_FAIL FALSE
  PASS_REGULAR_EXPRESSION "all tests passed"
  FAIL_REGULAR_EXPRESSION "panic|fatal"
  LABELS "unit;unicode"
  ENVIRONMENT "LC_ALL=C.UTF-8"
  WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}"
)
install(TARGETS atlas atlas_cli
  EXPORT AtlasTargets
  ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
  LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
  RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
  PUBLIC_HEADER DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}/atlas
)
install(FILES ${ATLAS_HEADERS} DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}/atlas)
install(EXPORT AtlasTargets NAMESPACE Atlas:: DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/Atlas)
export(EXPORT AtlasTargets FILE "${CMAKE_CURRENT_BINARY_DIR}/AtlasTargets.cmake")

set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -fno-omit-frame-pointer")
set(CMAKE_EXE_LINKER_FLAGS_DEBUG "${CMAKE_EXE_LINKER_FLAGS_DEBUG}")
set(EXECUTABLE_OUTPUT_PATH "${CMAKE_CURRENT_BINARY_DIR}/legacy-bin")
set(LIBRARY_OUTPUT_PATH "${CMAKE_CURRENT_BINARY_DIR}/legacy-lib")
set(BUILD_NAME "${CMAKE_SYSTEM_NAME}-${CMAKE_CXX_COMPILER_ID}")
message(STATUS "Legacy build name: ${BUILD_NAME}")
