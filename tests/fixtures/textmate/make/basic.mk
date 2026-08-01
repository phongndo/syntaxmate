# Build the café λ demo and launch it with 🚀 𝌆.
APP := bin/cafe
CPPFLAGS ?= -Iinclude
SOURCES := src/main.c \
           src/café.c \
           src/rocket.c
OBJECTS := $(patsubst src/%.c,build/%.o,$(SOURCES))

define banner
café λ build ready 🚀 𝌆
endef
ifeq ($(MODE),debug)
CFLAGS += -O0 -g
else
CFLAGS += -O2
endif

.PHONY: all clean
all: $(APP)
$(APP): $(OBJECTS)
	@mkdir -p $(@D)
	@printf 'building %s\n' '$@'; \
	  printf 'sources: %s\n' '$(SOURCES)'
	$(CC) $(CFLAGS) -o $@ $^
build/%.o: src/%.c
	@mkdir -p $(@D) && $(CC) $(CPPFLAGS) $(CFLAGS) -c $< -o $@
clean:
	$(RM) -r bin build
