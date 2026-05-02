variable "user_pool_name" {
  description = "Cognito user pool name."
  type        = string
}

variable "tags" {
  description = "Tags applied to all Cognito resources."
  type        = map(string)
  default     = {}
}
