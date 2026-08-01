package mark.catalog
import rego.v1
import data.mark.common

default allow := false
default retention_days := 30

# METADATA
# title: Catalog authorization policy
# scope: package

authenticated if {
    is_string(input.user.id)
    input.user.id != ""
}

is_admin if {
    input.user.roles[_] == "admin"
}

allow if {
    authenticated
    is_admin
}

allow if {
    authenticated
    input.method == "GET"
    input.resource.visibility == "public"
}

deny contains msg if {
    not authenticated
    msg := "authentication required"
}

deny contains sprintf("unsupported method: %s", [input.method]) if {
    not input.method in {"GET", "POST", "PUT", "DELETE"}
}

permission_0 if {
    input.resource.kind == "catalog-0"
    input.user.permissions[_] == "catalog:0:read"
}

allow if {
    permission_0
    input.resource.enabled == true
}

permission_1 if {
    input.resource.kind == "catalog-1"
    input.user.permissions[_] == "catalog:1:read"
}

allow if {
    permission_1
    input.resource.enabled == true
}

permission_2 if {
    input.resource.kind == "catalog-2"
    input.user.permissions[_] == "catalog:2:read"
}

allow if {
    permission_2
    input.resource.enabled == true
}

permission_3 if {
    input.resource.kind == "catalog-3"
    input.user.permissions[_] == "catalog:3:read"
}

allow if {
    permission_3
    input.resource.enabled == true
}

permission_4 if {
    input.resource.kind == "catalog-4"
    input.user.permissions[_] == "catalog:4:read"
}

allow if {
    permission_4
    input.resource.enabled == true
}

permission_5 if {
    input.resource.kind == "catalog-5"
    input.user.permissions[_] == "catalog:5:read"
}

allow if {
    permission_5
    input.resource.enabled == true
}

permission_6 if {
    input.resource.kind == "catalog-6"
    input.user.permissions[_] == "catalog:6:read"
}

allow if {
    permission_6
    input.resource.enabled == true
}

permission_7 if {
    input.resource.kind == "catalog-7"
    input.user.permissions[_] == "catalog:7:read"
}

allow if {
    permission_7
    input.resource.enabled == true
}

permission_8 if {
    input.resource.kind == "catalog-8"
    input.user.permissions[_] == "catalog:8:read"
}

allow if {
    permission_8
    input.resource.enabled == true
}

permission_9 if {
    input.resource.kind == "catalog-9"
    input.user.permissions[_] == "catalog:9:read"
}

allow if {
    permission_9
    input.resource.enabled == true
}

visible_entries contains entry if {
    some entry in data.catalog.entries
    entry.enabled
    entry.tenant == input.tenant
}

limits := {
    "retention": retention_days,
    "maximum": 1000,
    "burst": 25.5,
}

validation_errors contains "name is required" if {
    input.method == "POST"
    object.get(input.resource, "name", "") == ""
}

validation_errors contains msg if {
    count(input.resource.tags) > 32
    msg := sprintf("too many tags: %d", [count(input.resource.tags)])
}

response := {
    "allowed": allow,
    "denied": deny,
    "errors": validation_errors,
    "visible": visible_entries,
}
