# GNU Make grammar stress fixture; parsing is the goal, not execution.
# Unicode coverage: café, λ-calculus, 中文, 日本語, and emoji 😀🚀.

# Assignment flavors, modifiers, computed names, and continuations.
empty :=
PROJECT = mark
MODE ?= debug
HASH_TEXT := value\#fragment # inline comment after an escaped hash
IMMEDIATE ::= simple-value
DEFERRED :::= escaped-$$-value
SOURCES = main.c util.c café.c
SOURCES += generated.c
OBJECTS := $(SOURCES:.c=.o)
PROGRAMS := client server
client_OBJS := client.o net.o
server_OBJS := server.o net.o
name = DYNAMIC
$(name)_VALUE := computed-name
MULTILINE = first \
            second \
            third
SHELL_RESULT != printf '%s' safe-shell-assignment
override OVERRIDE_NAME := makefile-wins
export EXPORTED_NAME := visible
export PATH_COPY = $(PATH)
unexport SECRET_NAME
private PRIVATE_NOTE := hidden
OBSOLETE = remove-me
undefine OBSOLETE
# define/endef forms preserve literal dollars and recipe tabs.
define banner
project=$(PROJECT)
unicode=café λ 中文 😀
literal=$$HOME
endef
define SIMPLE_BLOCK :=
alpha
beta
endef
override define OVERRIDE_BLOCK =
one $(MODE)
two $$(later)
endef
define emit_recipe
	@printf '%s\n' 'generated target: $@ from $<'
endef

# Conditional directive spellings and nested branches.
ifeq ($(strip $(MODE)),debug)
CPPFLAGS += -DDEBUG=1
else ifeq "$(MODE)" "release"
CPPFLAGS += -DNDEBUG=1
else
CPPFLAGS += -DMODE_UNKNOWN=1
endif
ifneq '$(PROJECT)' ''
HAS_PROJECT := yes
endif
ifdef EXPORTED_NAME
EXPORT_STATE := defined
ifndef MISSING_NAME
MISSING_STATE := absent
endif
endif
# Include directives intentionally expand to no required files.
include $(wildcard stress-local.mk)
-include stress-optional.mk generated/dependencies.mk
sinclude stress-site.mk

# Built-in functions, references, substitutions, and introspection.
trimmed := $(strip   padded words   )
replaced := $(subst .c,.o,$(SOURCES))
patterned := $(patsubst %.c,build/%.o,$(SOURCES))
matched := $(filter %.c café.%,$(SOURCES))
rejected := $(filter-out generated.%,$(SOURCES))
ordered := $(sort zeta alpha λ alpha)
located := $(findstring café,$(SOURCES))
piece := $(word 2,$(SOURCES))
pieces := $(wordlist 1,3,$(SOURCES))
count := $(words $(SOURCES))
paths := src/main.c lib/util.c ./café.c
dirs := $(dir $(paths))
files := $(notdir $(paths))
suffixes := $(suffix $(paths))
stems := $(basename $(paths))
headers := $(addsuffix .h,$(basename $(files)))
prefixed := $(addprefix obj/,$(OBJECTS))
wild := $(wildcard *.mk)
choose = $(if $(strip $(1)),$(1),fallback)
decorate = [$(1):$(2)]
chosen := $(call choose,$(MODE))
label := $(call decorate,café,λ)
looped := $(foreach item,$(PROGRAMS),$(item):$($(item)_OBJS))
boolean := $(if $(and $(PROJECT),$(MODE)),both,$(or $(PROJECT),none))
raw_sources := $(value SOURCES)
sources_origin := $(origin SOURCES)
sources_flavor := $(flavor SOURCES)
missing_origin := $(origin DOES_NOT_EXIST)
# eval creates ordinary rules while retaining nested expansion syntax.
define make_alias
$(1): $(2)
	@printf 'alias %s <- %s\n' '$$@' '$$<'
endef
$(foreach p,$(PROGRAMS),$(eval $(call make_alias,$(p)-alias,$(p))))
# Global special targets and directive-like declarations.
.DEFAULT_GOAL := all
.SECONDEXPANSION:
.DELETE_ON_ERROR:
.NOTPARALLEL: serial-one serial-two
.PRECIOUS: %.keep
.SECONDARY: cached.out
.PHONY: all prep clean show-vars force docs serial-one serial-two
# Target- and pattern-specific variables, including modifiers.
app: CFLAGS := -O2 -Wall
app: private LDLIBS += -lm
app: export RUN_MODE = fixture
%.o: CFLAGS += -fPIC
%.o: private LOCAL_FLAG := object-only
build/%.o: CPPFLAGS := -Iinclude
# Explicit rules, order-only prerequisites, and automatic variables.
all: prep app archive.a docs multi-a multi-b grouped-a grouped-b double prefixed
	@printf '%s\n' 'all complete 😀'
prep: | build/
	@printf 'prepare %s; order-only=%s\n' '$@' '$|'
app: main.o util.o | build/
	@printf 'target=%s first=%s newer=%s\n' '$@' '$<' '$?'
	@printf 'unique=%s all=%s order=%s\n' '$^' '$+' '$|'
build/:
	@printf '%s\n' 'would create build directory'
# Implicit pattern rules and a static pattern rule.
%.o: %.c include/common.h | build/
	@printf 'compile stem=%s target=%s source=%s\n' '$*' '$@' '$<'
	@printf '%s\n' "flags: $(CPPFLAGS) $(CFLAGS) $(LOCAL_FLAG)"
build/%.o: src/%.c
	$(emit_recipe)
STATIC_OBJECTS := alpha.o beta.o
$(STATIC_OBJECTS): %.o: %.c
	@printf 'static pattern %s from %s\n' '$@' '$<'
# Multiple independent targets and GNU grouped targets.
multi-a multi-b: common.in
	@printf 'independent target %s\n' '$@'
grouped-a grouped-b &: grouped.in
	@printf 'grouped target %s, peers updated together\n' '$@'
# Double-colon, archive-member, suffix, and empty recipes.
double:: input-one
	@printf 'double-colon first: %s\n' '$<'
double:: input-two
	@printf 'double-colon second: %s\n' '$<'
archive.a(member.o): member.c
	@printf 'archive=%s member=%s stem=%s\n' '$@' '$%' '$*'
.c.o:
	@printf 'suffix rule %s -> %s\n' '$<' '$@'

noop: ; @printf '%s\n' 'inline empty-style recipe'
force: ;
# Secondary expansion uses the eventual automatic target variable.
EXTRA_second := extra.dat
second: $$(EXTRA_$$@)
	@printf 'secondary prerequisites: %s\n' '$^'

# Shell quoting, escaped dollars, Unicode, and physical continuations.
docs:
	@printf '%s\n' "double quoted: café λ 中文 😀" \
	  'single quoted: $$HOME stays shell text'
	@who='Ada Lovelace'; \
	  printf 'hello %s / %s\n' "$$who" "$${LANG:-unset}"
	@printf 'command substitution: %s\n' "$$(printf '%s' safe)"
	@printf '%s\n' 'a # inside recipe quotes is not a make comment'

serial-one serial-two:
	+@printf 'recursive-command marker on %s\n' '$@'
	-@printf '%s\n' 'ignored-status marker without failure'

# A temporary custom recipe prefix, then restoration to tab recipes.
.RECIPEPREFIX := >
prefixed:
>@printf '%s\n' 'custom recipe prefix >'
.RECIPEPREFIX :=

show-vars:
	@printf 'banner=%s\n' '$(banner)'
	@printf 'origin=%s flavor=%s raw=%s\n' \
	  '$(sources_origin)' '$(sources_flavor)' '$(raw_sources)'
	@printf 'unicode variable: %s\n' '$(label) 日本語 🚀'

# Safe cleanup demonstration: print only; never remove fixture data.
clean:
	@printf '%s\n' 'would remove generated outputs (dry fixture only)'
