# Builds the Syntaxmate C library and runs the C and C++ tests.
#
#   make check            header check + tests under ASan/UBSan/LSan
#   make header           regenerate include/syntaxmate.h (needs cbindgen)
#   make valgrind         tests without sanitizers under valgrind
#   make install PREFIX=/usr/local  install library, headers, pkg-config, CMake config
#   make check-install    install to a scratch prefix, check the exported symbols, and build
#                         the C test via pkg-config and CMake (and, on Linux, check that
#                         the installed prefix can move)
#   make PROFILE=release  use the release library
#   make TARGET_DIR=/path  build in another Cargo target directory
#   make bench            C/C++ vs native Rust throughput (always release; see README.md)
#
# Needs cargo, a C99 and C++17 compiler, and cbindgen for the header targets.

PROFILE ?= debug
UNAME := $(shell uname -s)
CARGO ?= cargo
CBINDGEN ?= cbindgen
VALGRIND ?= valgrind
READELF ?= readelf
CMAKE ?= cmake
CC ?= cc
CXX ?= c++

WORKSPACE := $(abspath ..)
# Cargo builds here too; defaults to $CARGO_TARGET_DIR when that is set.
TARGET_DIR ?= $(or $(CARGO_TARGET_DIR),$(WORKSPACE)/target)
override TARGET_DIR := $(abspath $(TARGET_DIR))
LIBDIR := $(TARGET_DIR)/$(PROFILE)
BUILD := $(TARGET_DIR)/c-tests/$(PROFILE)
CARGO_FLAGS := $(if $(filter release,$(PROFILE)),--release,)

ifeq ($(UNAME),Darwin)
NATIVE_LIBS ?=
SHARED_LIB := libsyntaxmate.dylib
EXPORTS ?= nm -gU
else
NATIVE_LIBS ?= -lpthread -ldl -lm
SHARED_LIB := libsyntaxmate.so
EXPORTS ?= nm -D --defined-only
endif

# Randomized test iterations per run; FUZZ_ITERATIONS="N SEED" replays a seed.
FUZZ_ITERATIONS ?= 300
SANITIZE ?= -fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=all
WARNINGS := -Wall -Wextra -Wpedantic -Werror
CFLAGS_TEST := -std=c99 $(WARNINGS) -g -Iinclude
CXXFLAGS_TEST := -std=c++17 $(WARNINGS) -g -Iinclude
# Apple's toolchain has no LeakSanitizer, and ASan aborts if asked for it.
ifeq ($(UNAME),Darwin)
export ASAN_OPTIONS ?= detect_leaks=0
else
export ASAN_OPTIONS ?= detect_leaks=1
endif
export UBSAN_OPTIONS ?= print_stacktrace=1

PREFIX ?= /usr/local
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' $(WORKSPACE)/Cargo.toml | head -n 1)

.PHONY: check lib test valgrind header check-header install check-install bench clean

check: check-header test check-profile

lib:
	$(CARGO) build -p syntaxmate-ffi $(CARGO_FLAGS) --manifest-path $(WORKSPACE)/Cargo.toml \
		--target-dir $(TARGET_DIR)

# The C test links the shared library; the C++ tests link the static one.
$(BUILD)/%/test_c: tests/test_c.c include/syntaxmate.h lib
	@mkdir -p $(@D)
	$(CC) $(CFLAGS_TEST) $(FLAGS) $< -o $@ -L$(LIBDIR) -lsyntaxmate -Wl,-rpath,$(LIBDIR) -lpthread

$(BUILD)/%/test_fuzz: tests/test_fuzz.c include/syntaxmate.h lib
	@mkdir -p $(@D)
	$(CC) $(CFLAGS_TEST) $(FLAGS) $< -o $@ -L$(LIBDIR) -lsyntaxmate -Wl,-rpath,$(LIBDIR)

$(BUILD)/%/test_cpp: tests/test_cpp.cpp include/syntaxmate.hpp include/syntaxmate.h lib
	@mkdir -p $(@D)
	$(CXX) $(CXXFLAGS_TEST) $(FLAGS) $< -o $@ $(LIBDIR)/libsyntaxmate.a $(NATIVE_LIBS)

$(BUILD)/%/test_cpp_alloc: tests/test_cpp_alloc.cpp include/syntaxmate.hpp include/syntaxmate.h lib
	@mkdir -p $(@D)
	$(CXX) $(CXXFLAGS_TEST) $(FLAGS) $< -o $@ $(LIBDIR)/libsyntaxmate.a $(NATIVE_LIBS)

test:
	$(MAKE) --no-print-directory run VARIANT=sanitize FLAGS="$(SANITIZE)" RUNNER=

valgrind:
	$(MAKE) --no-print-directory run VARIANT=plain FLAGS= \
		RUNNER="$(VALGRIND) --error-exitcode=1 --leak-check=full --errors-for-leak-kinds=definite,indirect"

.PHONY: run
run: $(BUILD)/$(VARIANT)/test_c $(BUILD)/$(VARIANT)/test_fuzz $(BUILD)/$(VARIANT)/test_cpp \
		$(BUILD)/$(VARIANT)/test_cpp_alloc
	$(RUNNER) $(BUILD)/$(VARIANT)/test_c
	$(RUNNER) $(BUILD)/$(VARIANT)/test_fuzz $(FUZZ_ITERATIONS)
	$(RUNNER) $(BUILD)/$(VARIANT)/test_cpp $(WORKSPACE)/conformance
	$(RUNNER) $(BUILD)/$(VARIANT)/test_cpp_alloc

header:
	$(CBINDGEN) --config cbindgen.toml --output include/syntaxmate.h .

check-header:
	@mkdir -p $(TARGET_DIR)
	$(CBINDGEN) --config cbindgen.toml --output $(TARGET_DIR)/syntaxmate.h.check .
	@diff -u include/syntaxmate.h $(TARGET_DIR)/syntaxmate.h.check \
		|| { echo "include/syntaxmate.h is stale; run 'make header'" >&2; exit 1; }

install: lib
	install -d $(DESTDIR)$(PREFIX)/lib/pkgconfig $(DESTDIR)$(PREFIX)/lib/cmake/syntaxmate $(DESTDIR)$(PREFIX)/include
	install -m 644 include/syntaxmate.h include/syntaxmate.hpp $(DESTDIR)$(PREFIX)/include/
	install -m 644 $(LIBDIR)/libsyntaxmate.a $(DESTDIR)$(PREFIX)/lib/
	install -m 755 $(LIBDIR)/$(SHARED_LIB) $(DESTDIR)$(PREFIX)/lib/
	install -m 644 cmake/syntaxmateConfig.cmake $(DESTDIR)$(PREFIX)/lib/cmake/syntaxmate/
	sed -e 's|@VERSION@|$(VERSION)|' cmake/syntaxmateConfigVersion.cmake.in \
		> $(DESTDIR)$(PREFIX)/lib/cmake/syntaxmate/syntaxmateConfigVersion.cmake
	sed -e 's|@PREFIX@|$(PREFIX)|' -e 's|@VERSION@|$(VERSION)|' -e 's|@NATIVE_LIBS@|$(NATIVE_LIBS)|' \
		syntaxmate.pc.in > $(DESTDIR)$(PREFIX)/lib/pkgconfig/syntaxmate.pc

check-install:
	rm -rf $(BUILD)/prefix $(BUILD)/relocated $(BUILD)/cmake $(BUILD)/cmake-mismatch $(BUILD)/cmake-nolib $(BUILD)/nolib
	$(MAKE) --no-print-directory install PREFIX=$(BUILD)/prefix
	@# The shared library exports the C API and nothing else.
	$(EXPORTS) $(BUILD)/prefix/lib/$(SHARED_LIB) | awk 'NF >= 3 { print $$3 }' > $(BUILD)/exports
	grep -q '^_\{0,1\}sm_version$$' $(BUILD)/exports
	! grep -v '^_\{0,1\}sm_' $(BUILD)/exports
	PKG_CONFIG_PATH=$(BUILD)/prefix/lib/pkgconfig sh -c '$(CC) -std=c99 tests/test_c.c -o $(BUILD)/prefix/test_c \
		$$(pkg-config --cflags --libs syntaxmate) -Wl,-rpath,$(BUILD)/prefix/lib -lpthread'
	$(BUILD)/prefix/test_c
	$(CMAKE) -S tests/cmake -B $(BUILD)/cmake -DCMAKE_PREFIX_PATH=$(BUILD)/prefix -DCMAKE_C_COMPILER=$(CC) \
		-DSYNTAXMATE_VERSION=$(basename $(VERSION))
	$(CMAKE) --build $(BUILD)/cmake
	$(BUILD)/cmake/test_c
	@# A request for another minor version must be rejected before 1.0.
	! $(CMAKE) -S tests/cmake -B $(BUILD)/cmake-mismatch -DCMAKE_PREFIX_PATH=$(BUILD)/prefix \
		-DCMAKE_C_COMPILER=$(CC) -DSYNTAXMATE_VERSION=0.999 > $(BUILD)/cmake-mismatch.log 2>&1
	grep -q 'version: $(VERSION)' $(BUILD)/cmake-mismatch.log
	@# A prefix without the library must fail at configure time, not link time.
	rm -rf $(BUILD)/nolib && cp -R $(BUILD)/prefix $(BUILD)/nolib && rm -f $(BUILD)/nolib/lib/libsyntaxmate.*
	! $(CMAKE) -S tests/cmake -B $(BUILD)/cmake-nolib -DCMAKE_PREFIX_PATH=$(BUILD)/nolib \
		-DCMAKE_C_COMPILER=$(CC) -DSYNTAXMATE_VERSION=$(basename $(VERSION)) > $(BUILD)/cmake-nolib.log 2>&1
	grep -q 'libsyntaxmate was not found' $(BUILD)/cmake-nolib.log
ifeq ($(UNAME),Linux)
	@# The SONAME keeps the install path out of consumers that link the
	@# library by path, as CMake does; such a program must survive a move.
	$(READELF) -d $(BUILD)/prefix/lib/libsyntaxmate.so | grep -q 'SONAME.*\[libsyntaxmate\.so\]'
	@# The README's static recipe must not pull in the shared library.
	$(CC) -std=c99 -I$(BUILD)/prefix/include tests/test_c.c -o $(BUILD)/prefix/test_c_static \
		$(BUILD)/prefix/lib/libsyntaxmate.a -lpthread -ldl -lm
	$(BUILD)/prefix/test_c_static
	! $(READELF) -d $(BUILD)/prefix/test_c_static | grep -q 'libsyntaxmate'
	mkdir -p $(BUILD)/prefix/bin
	$(CC) -std=c99 -I$(BUILD)/prefix/include tests/test_c.c -o $(BUILD)/prefix/bin/test_c \
		$(BUILD)/prefix/lib/libsyntaxmate.so '-Wl,-rpath,$$ORIGIN/../lib' -lpthread
	mv $(BUILD)/prefix $(BUILD)/relocated
	$(BUILD)/relocated/bin/test_c
endif

# Stress fixtures of the competitive benchmark's default languages.
BENCH_LANGUAGES ?= bash cpp html java json markdown python rust typescript yaml
BENCH_SAMPLES ?= 11
BENCH_MIN_MS ?= 200
BENCH_FIXTURES = $(foreach l,$(BENCH_LANGUAGES),$(l)=$(firstword $(filter-out %.jsonl,$(wildcard $(WORKSPACE)/../tests/fixtures/textmate/$(l)/stress.*))))
BENCH_CXXFLAGS ?= -O2

bench:
	$(MAKE) --no-print-directory lib PROFILE=release
	$(CARGO) build --release -p syntaxmate-ffi --example bench-native \
		--manifest-path $(WORKSPACE)/Cargo.toml --target-dir $(TARGET_DIR)
	@mkdir -p $(TARGET_DIR)/c-tests/bench
	$(CXX) -std=c++17 $(BENCH_CXXFLAGS) -Iinclude bench/bench.cpp -o $(TARGET_DIR)/c-tests/bench/bench \
		-L$(TARGET_DIR)/release -lsyntaxmate -Wl,-rpath,$(TARGET_DIR)/release -lpthread
	@echo "== native Rust (syntaxmate-boundary)"
	$(TARGET_DIR)/release/examples/bench-native $(BENCH_SAMPLES) $(BENCH_MIN_MS) $(BENCH_FIXTURES)
	@echo "== C ABI and C++ wrapper (libsyntaxmate)"
	$(TARGET_DIR)/c-tests/bench/bench $(BENCH_SAMPLES) $(BENCH_MIN_MS) $(BENCH_FIXTURES)

clean:
	rm -rf $(TARGET_DIR)/c-tests $(TARGET_DIR)/syntaxmate.h.check

# Uninstrumented release consumer timing; see README.md for timing boundaries.
.PHONY: profile check-profile
profile:
	$(MAKE) --no-print-directory PROFILE=release $(TARGET_DIR)/c-tests/release/profile

$(TARGET_DIR)/c-tests/release/profile: bench/profile.cpp include/syntaxmate.hpp include/syntaxmate.h lib
	@mkdir -p $(@D)
	$(CXX) -std=c++17 $(WARNINGS) -O3 -DNDEBUG -Iinclude $< -o $@ \
		-L$(LIBDIR) -lsyntaxmate -Wl,-rpath,$(LIBDIR) $(NATIVE_LIBS)

check-profile: $(BUILD)/test_profile
	$(BUILD)/test_profile

$(BUILD)/test_profile: tests/test_profile.cpp bench/profile.cpp include/syntaxmate.hpp include/syntaxmate.h lib
	@mkdir -p $(@D)
	$(CXX) $(CXXFLAGS_TEST) -O2 $< -o $@ \
		-L$(LIBDIR) -lsyntaxmate -Wl,-rpath,$(LIBDIR) $(NATIVE_LIBS)
