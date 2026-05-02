variable "env" {
  description = "Environment short name (e.g. dev, prod)."
  type        = string
}

variable "region" {
  description = "AWS region for all resources."
  type        = string
}

variable "project" {
  description = "Project prefix used in resource names."
  type        = string
  default     = "ki-do-timer"
}

variable "lambda_zip_path" {
  description = "Path to the cargo-lambda built deployment zip."
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
