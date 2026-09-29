# Builds the Syntaxmate C library and runs the C and C++ tests.
#
#   make check            header check + tests under ASan/UBSan/LSan
#   make header           regenerate include/syntaxmate.h (needs cbindgen)
#   make valgrind         tests without sanitizers under valgrind
#   make install PREFIX=/usr/local  install library, headers, pkg-config, CMake config
#   make check-install    install to a scratch prefix and build the C test via pkg-config
#                         (and, on Linux, check that the installed prefix can move)
#   make PROFILE=release  use the release library
#   make TARGET_DIR=/path  build in another Cargo target directory
#
# Needs cargo, a C99 and C++17 compiler, and cbindgen for the header targets.

PROFILE ?= debug
CARGO ?= cargo
CBINDGEN ?= cbindgen
VALGRIND ?= valgrind
READELF ?= readelf
CC ?= cc
CXX ?= c++

WORKSPACE := $(abspath ..)
# Cargo builds here too; defaults to $CARGO_TARGET_DIR when that is set.
TARGET_DIR ?= $(or $(CARGO_TARGET_DIR),$(WORKSPACE)/target)
override TARGET_DIR := $(abspath $(TARGET_DIR))
LIBDIR := $(TARGET_DIR)/$(PROFILE)
BUILD := $(TARGET_DIR)/c-tests/$(PROFILE)
CARGO_FLAGS := $(if $(filter release,$(PROFILE)),--release,)

ifeq ($(shell uname -s),Darwin)
NATIVE_LIBS ?=
else
NATIVE_LIBS ?= -lpthread -ldl -lm
endif

SANITIZE ?= -fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=all
WARNINGS := -Wall -Wextra -Wpedantic -Werror
CFLAGS_TEST := -std=c99 $(WARNINGS) -g -Iinclude
CXXFLAGS_TEST := -std=c++17 $(WARNINGS) -g -Iinclude
export ASAN_OPTIONS ?= detect_leaks=1
export UBSAN_OPTIONS ?= print_stacktrace=1

PREFIX ?= /usr/local
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' $(WORKSPACE)/Cargo.toml | head -n 1)

.PHONY: check lib test valgrind header check-header install check-install clean

check: check-header test

lib:
	$(CARGO) build -p syntaxmate-ffi $(CARGO_FLAGS) --manifest-path $(WORKSPACE)/Cargo.toml \
		--target-dir $(TARGET_DIR)

# The C test links the shared library; the C++ tests link the static one.
$(BUILD)/%/test_c: tests/test_c.c include/syntaxmate.h lib
	@mkdir -p $(@D)
	$(CC) $(CFLAGS_TEST) $(FLAGS) $< -o $@ -L$(LIBDIR) -lsyntaxmate -Wl,-rpath,$(LIBDIR) -lpthread

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
run: $(BUILD)/$(VARIANT)/test_c $(BUILD)/$(VARIANT)/test_cpp $(BUILD)/$(VARIANT)/test_cpp_alloc
	$(RUNNER) $(BUILD)/$(VARIANT)/test_c
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
	install -m 755 $(wildcard $(LIBDIR)/libsyntaxmate.so $(LIBDIR)/libsyntaxmate.dylib) $(DESTDIR)$(PREFIX)/lib/
	install -m 644 cmake/syntaxmateConfig.cmake $(DESTDIR)$(PREFIX)/lib/cmake/syntaxmate/
	sed -e 's|@PREFIX@|$(PREFIX)|' -e 's|@VERSION@|$(VERSION)|' -e 's|@NATIVE_LIBS@|$(NATIVE_LIBS)|' \
		syntaxmate.pc.in > $(DESTDIR)$(PREFIX)/lib/pkgconfig/syntaxmate.pc

check-install:
	rm -rf $(BUILD)/prefix $(BUILD)/relocated
	$(MAKE) --no-print-directory install PREFIX=$(BUILD)/prefix
	PKG_CONFIG_PATH=$(BUILD)/prefix/lib/pkgconfig sh -c '$(CC) -std=c99 tests/test_c.c -o $(BUILD)/prefix/test_c \
		$$(pkg-config --cflags --libs syntaxmate) -Wl,-rpath,$(BUILD)/prefix/lib -lpthread'
	$(BUILD)/prefix/test_c
ifeq ($(shell uname -s),Linux)
	@# The SONAME keeps the install path out of consumers that link the
	@# library by path, as CMake does; such a program must survive a move.
	$(READELF) -d $(BUILD)/prefix/lib/libsyntaxmate.so | grep -q 'SONAME.*\[libsyntaxmate\.so\]'
	mkdir -p $(BUILD)/prefix/bin
	$(CC) -std=c99 -I$(BUILD)/prefix/include tests/test_c.c -o $(BUILD)/prefix/bin/test_c \
		$(BUILD)/prefix/lib/libsyntaxmate.so '-Wl,-rpath,$$ORIGIN/../lib' -lpthread
	mv $(BUILD)/prefix $(BUILD)/relocated
	$(BUILD)/relocated/bin/test_c
endif

clean:
	rm -rf $(TARGET_DIR)/c-tests $(TARGET_DIR)/syntaxmate.h.check
