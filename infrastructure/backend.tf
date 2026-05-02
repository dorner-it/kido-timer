# Reuses the existing fms-monorepo state bucket and lock table in the same
# AWS account. Provisioned out-of-band; we just put a different key here.
terraform {
  backend "s3" {
    bucket         = "dorner-it-fms-terraform-state"
    key            = "ki-do-timer/terraform.tfstate"
    region         = "eu-central-1"
    dynamodb_table = "fms-terraform-locks"
    encrypt        = true
  }
}
