variable "user_pool_name" {
  description = "Cognito user pool name."
  type        = string
}

variable "hosted_ui_domain_prefix" {
  description = "Cognito hosted UI subdomain prefix (must be globally unique within the AWS region)."
  type        = string
}

variable "callback_urls" {
  description = "OAuth2 callback URLs registered with the Cognito client. The backend serves these."
  type        = list(string)
}

variable "logout_urls" {
  description = "Post-logout redirect URLs registered with the Cognito client. The frontend hosts these."
  type        = list(string)
}

variable "tags" {
  description = "Tags applied to the user pool and domain."
  type        = map(string)
  default     = {}
}
