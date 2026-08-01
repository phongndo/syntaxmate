# Terraform/HCL TextMate stress fixture — Unicode, templates, and nested state 🚀 𝌆
terraform {
  required_version = ">= 1.6.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    random = { source = "hashicorp/random" }
  }
  backend "s3" {
    bucket         = "example-terraform-state"
    key            = "stress/terraform.tfstate"
    region         = "us-east-1"
  }
}

provider "aws" {
  region  = var.region
  profile = var.profile == "" ? null : var.profile
  default_tags {
    tags = merge(local.common_tags, { ManagedBy = "terraform" })
  }
}
provider "aws" {
  alias  = "replica"
  region = var.replica_region
}

variable "region" {
  description = "Primary deployment region 🌍"
  type        = string
  default     = "us-east-1"
  validation {
    condition     = can(regex("^[a-z]{2}-[a-z]+-[0-9]+$", var.region))
    error_message = "region must resemble us-east-1."
  }
}
variable "replica_region" {
  type    = string
  default = "eu-west-1"
}
variable "profile" {
  type      = string
  default   = ""
  nullable  = false
  sensitive = true
}
variable "services" {
  description = "Service definitions may use Unicode names such as café or 東京."
  type = map(object({
    image   = string
    ports   = set(number)
    labels  = map(string)
    command = optional(tuple([string, number]))
    health = optional(object({
      path     = string
      interval = number
    }))
  }))
  default = {
    api = {
      image   = "example/api:1.2.3"
      ports   = [8080, 8443]
      labels  = { tier = "backend", glyph = "🚀" }
      command = ["serve", 2]
      health  = { path = "/ready", interval = 30 }
    }
  }
}
variable "feature_flags" {
  type    = set(string)
  default = ["audit", "metrics"]
}

locals {
  /* Expressions deliberately combine traversals, indexing, and operators. */
  environment = terraform.workspace != "default" ? terraform.workspace : "dev"
  common_tags = {
    Environment = local.environment
    Project     = "grammar-stress-𝌆"
  }
  enabled_services = {
    for name, service in var.services : name => service
    if length(service.ports) > 0 && !contains(var.feature_flags, "disable-${name}")
  }
  ports_by_service = [for service in values(local.enabled_services) : sort(tolist(service.ports))]
  grouped_by_tier  = { for name, service in var.services : service.labels.tier => name... }
  flattened_ports  = distinct(flatten(local.ports_by_service))
  first_private_ip = try(aws_instance.worker[0].private_ip, "0.0.0.0")
  encoded_manifest = jsonencode({ services = local.enabled_services, flags = var.feature_flags })
  escaped_template = "literal $${not_interpolation}, percent %%{ untouched }, real ${upper(local.environment)}"
}

data "aws_caller_identity" "current" {}

data "aws_ami" "linux" {
  most_recent = true
  owners      = ["amazon"]
  filter {
    name   = "name"
    values = ["al2023-ami-*-x86_64"]
  }
}
resource "random_id" "suffix" {
  byte_length = 4
  keepers     = { environment = local.environment }
}
resource "aws_security_group" "service" {
  name        = "${local.environment}-${random_id.suffix.hex}"
  description = "Generated access for ${join(", ", keys(local.enabled_services))}"
  dynamic "ingress" {
    for_each = toset(local.flattened_ports)
    iterator = port
    content {
      description = "service port ${port.value}"
      from_port   = port.value
      to_port     = port.value
      protocol    = "tcp"
      cidr_blocks = ["10.0.0.0/8"]
    }
  }
  egress {
    from_port        = 0
    to_port          = 0
    protocol         = "-1"
    ipv6_cidr_blocks = ["::/0"]
  }
  lifecycle {
    create_before_destroy = true
    ignore_changes        = [description, tags["LastUpdated"]]
    precondition {
      condition     = 50 >= length(local.flattened_ports)
      error_message = "No more than 50 distinct ports are supported."
    }
  }
}
resource "aws_instance" "worker" {
  for_each               = local.enabled_services
  ami                    = data.aws_ami.linux.id
  instance_type          = lookup(each.value.labels, "size", "t3.micro")
  vpc_security_group_ids = [aws_security_group.service.id]
  # A multiline tuple and for expression build a shell template without heredoc state.
  user_data = join("\n", concat(
    ["#!/usr/bin/env bash", "set -euo pipefail"],
    [
      "echo \"service=${each.key}\" > /etc/service.env",
      "echo \"account=${data.aws_caller_identity.current.account_id}\" >> /etc/service.env",
    ],
    contains(var.feature_flags, "metrics") ? ["echo 'metrics=true'"] : ["echo 'metrics=false'"],
    [for port in sort(tolist(each.value.ports)) : "echo \"listen=${port}\" >> /etc/service.env"]
  ))
  tags = merge(local.common_tags, {
    Name    = format("%s-%s", local.environment, each.key)
    Service = each.key
  })
  lifecycle {
    replace_triggered_by = [random_id.suffix]
    postcondition {
      condition     = self.instance_state == "running"
      error_message = "Worker did not reach running state."
    }
  }
}

module "observability" {
  source  = "./modules/observability"
  providers = {
    aws         = aws
    aws.replica = aws.replica
  }
  service_ids = { for name, instance in aws_instance.worker : name => instance.id }
  config      = templatefile("${path.module}/templates/config.tftpl", local.enabled_services)
  depends_on  = [aws_security_group.service]
}

output "deployment_summary" {
  value = {
    account          = data.aws_caller_identity.current.account_id
    ids              = values(aws_instance.worker)[*].id
    grouped_services = local.grouped_by_tier
    rendered         = local.escaped_template
  }
  sensitive = false
  precondition {
    condition     = length(aws_instance.worker) > 0 || local.environment == "dev"
    error_message = "Non-development workspaces require at least one worker."
  }
}
output "operator_notes" {
  value = join("\n", concat(
    ["Deployment: ${local.environment} 🚀", "Primary IP: ${local.first_private_ip}"],
    [for tier, names in local.grouped_by_tier : "${title(tier)}: ${join(", ", names)}"],
    ["JSON: ${local.encoded_manifest}"]
  ))
}
