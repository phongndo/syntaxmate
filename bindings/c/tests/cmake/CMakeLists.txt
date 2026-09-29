# Builds tests/test_c.c against an installed prefix through the CMake package,
# as `make check-install` does: cmake -DCMAKE_PREFIX_PATH=<prefix> ...
cmake_minimum_required(VERSION 3.18)
project(syntaxmate_cmake_consumer C)

# make check-install passes the installed major.minor, and a mismatched one to
# check that the version file rejects it.
find_package(syntaxmate ${SYNTAXMATE_VERSION} REQUIRED)
add_executable(test_c ${CMAKE_CURRENT_SOURCE_DIR}/../test_c.c)
set_target_properties(test_c PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON)
target_link_libraries(test_c PRIVATE syntaxmate::syntaxmate)
