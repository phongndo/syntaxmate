# Terraform smoke fixture: café λ
variable "name" {
  type    = string
  default = "mark"
}

resource "null_resource" "example" {
  triggers = {
    name = var.name
  }
}
