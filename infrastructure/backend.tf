# Remote state.
#
# Backend blocks cannot interpolate variables, so we configure the S3 backend
# via a per-environment `-backend-config` file at `terraform init` time.
#
# Example backend-dev.hcl:
#
#   bucket         = "ki-do-timer-tfstate"
#   key            = "ki-do-timer/dev/terraform.tfstate"
#   region         = "eu-central-1"
#   dynamodb_table = "ki-do-timer-tflock"
#   encrypt        = true
#
# Then:
#
#   terraform init -backend-config=backend-dev.hcl
#
# Uncomment the block below once the state bucket and lock table are
# provisioned out-of-band:
#
# terraform {
#   backend "s3" {}
# }
