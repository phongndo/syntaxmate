cmake_minimum_required(VERSION 3.20)
project(GrammarFixture VERSION 1.2.3 LANGUAGES C CXX)

# A compact café / 日本語 fixture launched with 🚀 and 𝌆.
option(ENABLE_GREETING "Build the greeting target" ON)
set(GREETING "Bonjour, café! 日本語 🚀 𝌆")
set(SOURCES main.cpp greeting.cpp)

if(ENABLE_GREETING AND NOT WIN32)
  message(STATUS "${GREETING}")
elseif(DEFINED ENV{CI})
  message(WARNING "CI path: $ENV{PATH}")
else()
  message(STATUS "Greeting disabled")
endif()

set(HELP_TEXT [=[
Multiline help keeps "quotes", ${variables}, and Unicode café 🚀 literal.
]=])
#[[ A closed bracket comment: 日本語 𝌆 ]]
add_executable(greeter ${SOURCES})
target_compile_features(greeter PRIVATE cxx_std_20)
target_compile_definitions(greeter PRIVATE GREETING_TEXT="${GREETING}")
install(TARGETS greeter RUNTIME DESTINATION bin)
