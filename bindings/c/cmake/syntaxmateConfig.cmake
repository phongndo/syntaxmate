# Installed as <prefix>/lib/cmake/syntaxmate/syntaxmateConfig.cmake.
# Provides the imported target syntaxmate::syntaxmate (shared library if
# installed, else static).
include(CMakeFindDependencyMacro)
find_dependency(Threads)

get_filename_component(_syntaxmate_prefix "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)
if(NOT TARGET syntaxmate::syntaxmate)
  find_library(SYNTAXMATE_LIBRARY NAMES syntaxmate
    PATHS "${_syntaxmate_prefix}/lib" NO_DEFAULT_PATH REQUIRED)
  add_library(syntaxmate::syntaxmate UNKNOWN IMPORTED)
  set_target_properties(syntaxmate::syntaxmate PROPERTIES
    IMPORTED_LOCATION "${SYNTAXMATE_LIBRARY}"
    INTERFACE_INCLUDE_DIRECTORIES "${_syntaxmate_prefix}/include"
    # Needed only when the static library is selected.
    INTERFACE_LINK_LIBRARIES "Threads::Threads;${CMAKE_DL_LIBS};$<$<PLATFORM_ID:Linux>:m>")
endif()
unset(_syntaxmate_prefix)
