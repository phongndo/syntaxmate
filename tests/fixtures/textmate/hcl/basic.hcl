# Small, valid Terraform-style HCL with Unicode values.
terraform {
  required_version = ">= 1.5.0"
}

variable "service" {
  type = object({
    name    = string
    enabled = bool
    ports   = list(number)
  })
  default = {
    name    = "café-日本語-🚀-𝌆"
    enabled = true
    ports   = [80, 443]
  }
}

locals {
  label   = "service-${var.service.name}"
  primary = var.service.ports[0]
  summary = var.service.enabled ? upper(local.label) : "disabled"
}

output "summary" {
  value = local.summary
}
