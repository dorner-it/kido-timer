output "api_endpoint" {
  description = "Invoke URL of the API Gateway HTTP API."
  value       = module.api.api_endpoint
}

output "lambda_function_name" {
  value = module.api.lambda_function_name
}

output "table_name" {
  value = module.dynamodb.table_name
}

output "table_arn" {
  value = module.dynamodb.table_arn
}

output "jwt_signing_key_secret_arn" {
  value = module.secrets.jwt_signing_key_secret_arn
}

output "jwt_refresh_pepper_secret_arn" {
  value = module.secrets.jwt_refresh_pepper_secret_arn
}
