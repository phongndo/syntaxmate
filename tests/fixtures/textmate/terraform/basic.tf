# Terraform basic fixture: café, 東京, λ, 🚀, and astral 𝌆.
terraform { required_version = ">= 1.6.0" }
variable "region" {
  type        = string
  description = "Launch region"
  default     = "eu-west-1"
  validation {
    condition     = length(var.region) > 0
    error_message = "region must not be empty"
  }
}
locals {
  labels = {
    app   = "café-東京"
    orbit = "λ-🚀-𝌆"
  }
  ports   = [80, 443]
  message = "${local.labels.app} in ${var.region}"
}
resource "null_resource" "launch" {
  triggers = {
    region = var.region
    label  = local.labels.orbit
  }
}
output "summary" {
  value     = "${local.message}: ${join(",", local.ports)}"
  sensitive = false
}
