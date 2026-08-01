package mark.authz
import rego.v1

default allow := false

allow if {
    input.method == "GET"
    input.user.roles[_] == "reader"
}

allow if {
    input.method == "POST"
    input.user.roles[_] == "editor"
    startswith(input.path, "/catalog/")
}

deny contains "missing user" if {
    not input.user.id
}
