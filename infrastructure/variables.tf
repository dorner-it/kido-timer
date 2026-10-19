variable "env" {
  description = "Environment short name (e.g. dev, prod)."
  type        = string
}

variable "region" {
  description = "AWS region for all resources."
  type        = string
  default     = "eu-central-1"
}

variable "project" {
  description = "Project prefix used in resource names."
  type        = string
  default     = "ki-do-timer"
}

variable "github_repo" {
  description = "GitHub repository in 'owner/name' form. Used in the OIDC trust policy."
  type        = string
  default     = "dorner-it/kido-timer"
}

variable "github_repo_immutable" {
  description = "Immutable OIDC form 'owner@owner_id/name@repo_id' (see GET /repos/{owner}/{repo}/actions/oidc/customization/sub, sub_claim_prefix). Used in the OIDC trust policy."
  type        = string
  default     = "dorner-it@88201897/kido-timer@1425183376"
}

variable "lambda_zip_path" {
  description = "Path to the cargo-lambda built deployment zip (used only on the initial apply; CI updates code thereafter)."
  type        = string
  default     = "../backend/target/lambda/bootstrap/bootstrap.zip"
}

variable "cors_allowed_origins" {
  description = "Allowed CORS origins (Vercel domains). Empty list disables CORS configuration."
  type        = list(string)
  default     = []
}

variable "log_level" {
  description = "tracing-subscriber filter passed to the Lambda."
  type        = string
  default     = "info"
}

variable "log_retention_days" {
  description = "CloudWatch Log Group retention in days."
  type        = number
  default     = 14
}

variable "tags" {
  description = "Additional tags applied to all resources."
  type        = map(string)
  default     = {}
}
