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
  default     = "dorner-it/ki-do-timer-web"
}

variable "lambda_zip_path" {
  description = "Path to the cargo-lambda built deployment zip (used only on the initial apply; CI updates code thereafter)."
  type        = string
  default     = "../backend/target/lambda/bootstrap/bootstrap.zip"
}

variable "cognito_hosted_ui_domain_prefix" {
  description = "Cognito hosted UI subdomain prefix (must be globally unique within the AWS region)."
  type        = string
}

variable "cognito_callback_urls" {
  description = "OAuth2 callback URLs registered with the Cognito client. Hit by Cognito after login; backend handles them."
  type        = list(string)
  default     = ["http://localhost:9000/api/auth/callback"]
}

variable "cognito_logout_urls" {
  description = "Post-logout redirect URLs registered with the Cognito client. Frontend hosts these."
  type        = list(string)
  default     = ["http://localhost:4321/"]
}

variable "cognito_redirect_uri" {
  description = "The single callback URL the backend redirects to after Cognito login. Must be one of cognito_callback_urls."
  type        = string
  default     = "http://localhost:9000/api/auth/callback"
}

variable "frontend_base_url" {
  description = "Frontend base URL (used for post-login redirects)."
  type        = string
  default     = "http://localhost:4321"
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
