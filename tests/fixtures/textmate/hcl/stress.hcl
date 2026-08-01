# A broad Terraform-like fixture for the vendored HCL TextMate grammar.
terraform {
  required_version = ">= 1.6.0"
  required_providers {
    example = {
      source  = "example/example"
      version = "~> 2.4"
    }
  }
}

variable "names" {
  description = "Display names, including café, 日本語, 🚀, and 𝌆."
  type        = list(string)
  default     = ["Ada", "René", "日本語"]
  nullable    = false
}

variable "settings" {
  type = object({
    enabled = bool
    retries = number
    tags    = map(string)
    modes   = set(string)
    point   = tuple([number, number])
    extra   = any
  })
  default = {
    enabled = true
    retries = 3
    tags    = { environment = "test", owner = "platform" }
    modes   = ["safe", "fast"]
    point   = [12.5, -7]
    extra   = null
  }
}

variable "instances" {
  type = list(object({
    name = string
    port = number
  }))
  default = [
    { name = "alpha", port = 8080 },
    { name = "beta", port = 9090 },
  ]
}

variable "users" {
  type = map(object({
    name   = string
    active = bool
    roles  = list(string)
  }))
  default = {
    ada = {
      name   = "Ada"
      active = true
      roles  = ["admin", "author"]
    }
    ren = {
      name   = "René"
      active = false
      roles  = ["reader"]
    }
  }
}

locals {
  // Numeric forms and arithmetic operators.
  integer     = 42
  decimal     = 3.14159
  exponent    = 6.02E+23
  tiny        = 1e-9
  arithmetic  = ((local.integer + 8) * 2 - 4) / 3 % 7
  comparisons = local.integer >= 40 && local.integer <= 50
  unequal     = local.decimal != 0 || local.tiny == 0
  bounded     = local.integer > 0 && local.integer < 100
  negated     = !var.settings.enabled

  # Accessors, numeric indexes, full splats, and legacy attribute splats.
  first_name   = var.instances[0].name
  all_names    = var.instances[*].name
  legacy_names = var.instances.*.name
  tag_owner    = var.settings.tags.owner
  missing      = try(var.settings.tags.missing, null)

  /* Tuples, nested tuples,
     and a conditional expression. */
  coordinates = [var.settings.point[0], var.settings.point[1]]
  matrix      = [[1, 2], [3, 4], [5, 6]]
  selected    = var.settings.enabled ? local.coordinates : [0, 0]
  expanded    = concat([var.names, ["extra"]]...)

  upper_names = [for name in var.names : upper(name)]
  long_names  = [for name in var.names : name if length(name) > 3]
  user_labels = [for key, user in var.users : "${key}:${user.name}"]

  active_users = {
    for key, user in var.users : key => user
    if user.active
  }
  users_by_role = {
    for key, user in var.users : user.roles[0] => key...
  }

  # Object keys cover identifiers, quoted strings, and parenthesized expressions.
  dynamic_key = "generated"
  metadata = {
    simple              = true
    "quoted-key"        = "quoted value"
    (local.dynamic_key) = "dynamic value"
    nested = {
      count = length(var.names)
      valid = can(regex("^[[:alpha:]]+", var.names[0]))
    }
  }

  builtin_call    = format("%s-%02d", var.names[0], var.settings.retries)
  nested_call     = jsonencode(merge(var.settings.tags, { unicode = "café-日本語-🚀-𝌆" }))
  namespaced_call = provider::terraform::encode_tfvars({ names = var.names })
  escaped         = "quote: \" slash: \\ newline:\n tab:\t BMP:\u65E5 astral:\U0001F680"
  escaped_marker  = "literal $${not_interpolation} and %%{ not_a_directive }"
  interpolated    = "Hello, ${title(var.names[0])}; port=${var.instances[0].port}."
  directed        = "mode=%{ if var.settings.enabled }enabled%{ else }disabled%{ endif }"

  plain_document = <<DOC
Service ${var.instances[0].name}
Unicode café 日本語 🚀 𝌆
%{ if var.settings.enabled }
status = enabled
%{ else }
status = disabled
%{ endif }
DOC

  indented_document = <<-EOT
    Users:
    %{~ for key, user in var.users ~}
    - ${key}: ${user.name} (%{ if user.active }active%{ else }inactive%{ endif })
    %{ endfor ~}
    Literal-looking escape: $${user.name}
  EOT
}

resource "example_service" "primary" {
  name        = local.builtin_call
  description = local.interpolated
  enabled     = var.settings.enabled
  retries     = var.settings.retries
  endpoint    = "https://example.test/${var.instances[0].name}"
  payload     = local.plain_document

  tags = merge(
    var.settings.tags,
    {
      Name    = "café-${var.names[0]}"
      Unicode = "日本語-🚀-𝌆"
    },
  )

  dynamic "listener" {
    for_each = { for item in var.instances : item.name => item }
    content {
      name = listener.key
      port = listener.value.port
    }
  }

  lifecycle {
    create_before_destroy = true
    prevent_destroy       = false
    ignore_changes        = [tags["timestamp"]]
  }
}

data "example_catalog" "current" {
  filter = local.upper_names
}

module "consumer" {
  source = "./modules/consumer"

  service_id = example_service.primary.id
  names      = local.all_names
  options    = local.metadata
}

output "service_details" {
  description = "Computed details for café and 日本語 users 🚀 𝌆."
  value = {
    id       = example_service.primary.id
    endpoint = example_service.primary.endpoint
    users    = local.active_users
    healthy  = var.settings.enabled && var.settings.retries > 0
  }
  sensitive = false
}
