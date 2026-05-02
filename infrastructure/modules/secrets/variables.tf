variable "prefix" {
  description = "Resource name prefix (e.g. ki-do-timer-dev)."
  type        = string
}

variable "tags" {
  description = "Tags applied to all secrets."
  type        = map(string)
  default     = {}
}
