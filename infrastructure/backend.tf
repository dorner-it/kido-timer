# Reuses the existing fms-monorepo state bucket in the same AWS account
# (provisioned out-of-band; we just put a different key here). State
# locking uses S3-native lock files (`<key>.tflock`, Terraform >= 1.10)
# instead of the deprecated DynamoDB lock table.
terraform {
  backend "s3" {
    bucket       = "dorner-it-fms-terraform-state"
    key          = "ki-do-timer/terraform.tfstate"
    region       = "eu-central-1"
    use_lockfile = true
    encrypt      = true
  }
}
