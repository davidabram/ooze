variable "environment" {
  type    = string
  default = "dev"

  validation {
    condition     = contains(["dev", "staging", "prod"], var.environment)
    error_message = "Must be dev, staging, or prod."
  }
}

resource "aws_instance" "example" {
  count = var.environment == "prod" ? 3 : 1

  ami           = "ami-0c55b159cbfafe1f0"
  instance_type = "t2.micro"

  tags = {
    Name        = "ExampleInstance"
    Environment = var.environment
  }
}

locals {
  is_prod = var.environment == "prod"
  region  = var.environment == "prod" ? "us-east-1" : "us-west-2"

  env_upper = replace(var.environment, "d", "D")

  config = var.environment == "dev" ? {
    size = "small"
  } : var.environment == "staging" ? {
    size = "medium"
  } : {
    size = "large"
  }
}
