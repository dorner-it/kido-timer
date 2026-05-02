variable "function_name" {
  description = "Lambda function name."
  type        = string
}

variable "prefix" {
  description = "Resource name prefix."
  type        = string
}

variable "env" {
  description = "Environment short name passed to the Lambda."
  type        = string
}

variable "lambda_zip_path" {
  description = "Path to the cargo-lambda built deployment zip."
  type        = string
}

variable "table_name" {
  description = "DynamoDB table name passed to the Lambda."
  type        = string
}

variable "table_arn" {
  description = "DynamoDB table ARN used to scope IAM."
  type        = string
}

variable "cognito_user_pool_id" {
  description = "Cognito user pool id passed to the Lambda."
  type        = string
}

variable "cognito_user_pool_arn" {
  description = "Cognito user pool ARN used to scope IAM."
  type        = string
}

variable "cognito_client_id" {
  description = "Cognito user pool client id."
  type        = string
}

variable "cors_allowed_origins" {
  description = "Allowed CORS origins. Empty list disables the CORS configuration."
  type        = list(string)
  default     = []
}

variable "log_level" {
  description = "tracing-subscriber filter."
  type        = string
  default     = "info"
}

variable "log_retention_days" {
  description = "CloudWatch Log Group retention in days."
  type        = number
  default     = 14
}

variable "memory_size_mb" {
  description = "Lambda memory in MB."
  type        = number
  default     = 512
}

variable "timeout_seconds" {
  description = "Lambda timeout in seconds."
  type        = number
  default     = 10
}

variable "tags" {
  description = "Tags applied to all resources."
  type        = map(string)
  default     = {}
}
