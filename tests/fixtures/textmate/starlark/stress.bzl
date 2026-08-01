"""Comprehensive Starlark fixture for build rules and helpers."""
load("@bazel_skylib//lib:dicts.bzl", "dicts")
load("@rules_cc//cc:defs.bzl", "cc_binary", "cc_library", "cc_test")

DEFAULT_COPTS = ["-Wall", "-Wextra", "-Werror"]
PLATFORMS = {"linux": "@platforms//os:linux", "macos": "@platforms//os:macos"}

def _normalize_name(value):
    """Normalize a target fragment without using Python-only features."""
    return value.lower().replace("-", "_")

def _select_sources(prefix, names, excluded = []):
    selected = []
    for name in names:
        candidate = prefix + "/" + name
        if candidate not in excluded:
            selected.append(candidate)
    return selected

def _catalog_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".txt")
    content = "\n".join([file.short_path for file in ctx.files.srcs])
    ctx.actions.write(output = output, content = content)
    return [DefaultInfo(files = depset([output]))]

catalog_manifest = rule(
    implementation = _catalog_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "strict": attr.bool(default = True),
    },
)

cc_library(
    name = "catalog_0",
    srcs = _select_sources("src", ["entry_0.cc", "shared.cc"]),
    hdrs = ["include/entry_0.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=0"] if 0 > 0 else []),
    defines = ["MARK_CATALOG_0"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "catalog_1",
    srcs = _select_sources("src", ["entry_1.cc", "shared.cc"]),
    hdrs = ["include/entry_1.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=1"] if 1 > 0 else []),
    defines = ["MARK_CATALOG_1"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "catalog_2",
    srcs = _select_sources("src", ["entry_2.cc", "shared.cc"]),
    hdrs = ["include/entry_2.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=2"] if 2 > 0 else []),
    defines = ["MARK_CATALOG_2"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "catalog_3",
    srcs = _select_sources("src", ["entry_3.cc", "shared.cc"]),
    hdrs = ["include/entry_3.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=3"] if 3 > 0 else []),
    defines = ["MARK_CATALOG_3"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "catalog_4",
    srcs = _select_sources("src", ["entry_4.cc", "shared.cc"]),
    hdrs = ["include/entry_4.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=4"] if 4 > 0 else []),
    defines = ["MARK_CATALOG_4"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "catalog_5",
    srcs = _select_sources("src", ["entry_5.cc", "shared.cc"]),
    hdrs = ["include/entry_5.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=5"] if 5 > 0 else []),
    defines = ["MARK_CATALOG_5"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "catalog_6",
    srcs = _select_sources("src", ["entry_6.cc", "shared.cc"]),
    hdrs = ["include/entry_6.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=6"] if 6 > 0 else []),
    defines = ["MARK_CATALOG_6"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "catalog_7",
    srcs = _select_sources("src", ["entry_7.cc", "shared.cc"]),
    hdrs = ["include/entry_7.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=7"] if 7 > 0 else []),
    defines = ["MARK_CATALOG_7"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "catalog_8",
    srcs = _select_sources("src", ["entry_8.cc", "shared.cc"]),
    hdrs = ["include/entry_8.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=8"] if 8 > 0 else []),
    defines = ["MARK_CATALOG_8"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "catalog_9",
    srcs = _select_sources("src", ["entry_9.cc", "shared.cc"]),
    hdrs = ["include/entry_9.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=9"] if 9 > 0 else []),
    defines = ["MARK_CATALOG_9"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "catalog_10",
    srcs = _select_sources("src", ["entry_10.cc", "shared.cc"]),
    hdrs = ["include/entry_10.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=10"] if 10 > 0 else []),
    defines = ["MARK_CATALOG_10"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "catalog_11",
    srcs = _select_sources("src", ["entry_11.cc", "shared.cc"]),
    hdrs = ["include/entry_11.h"],
    copts = DEFAULT_COPTS + (["-DMARK_INDEX=11"] if 11 > 0 else []),
    defines = ["MARK_CATALOG_11"],
    deps = ["//base:status", "//base:strings"],
    visibility = ["//visibility:public"],
)

catalog_manifest(
    name = "manifest",
    srcs = glob(["src/**/*.cc"]),
)

cc_binary(
    name = "catalog_tool",
    srcs = ["tools/main.cc"],
    deps = [":catalog_0", ":catalog_1"],
)

cc_test(
    name = "catalog_test",
    srcs = ["tests/catalog_test.cc"],
    deps = [":catalog_0"],
)
