"""Small Starlark rule fixture."""
load("@rules_cc//cc:defs.bzl", "cc_library")

def _labels(prefix, values):
    return [prefix + value for value in values if value]

cc_library(
    name = "catalog",
    srcs = glob(["src/**/*.cc"], exclude = ["src/**/*_test.cc"]),
    hdrs = ["include/catalog.h"],
    defines = ["MARK_ENABLED=1"],
    deps = ["//base:status"],
    visibility = ["//visibility:public"],
)

CATALOG_LABELS = _labels("//catalog:", ["core", "tools"])
print("configured %d labels" % len(CATALOG_LABELS))
