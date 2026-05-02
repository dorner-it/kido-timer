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

output "cognito_user_pool_id" {
  value = module.cognito.user_pool_id
}

output "cognito_client_id" {
  value = module.cognito.client_id
}

output "cognito_issuer_url" {
  value = module.cognito.issuer_url
}

output "github_actions_terraform_role_arn" {
  value = aws_iam_role.github_actions_terraform.arn
}

output "github_actions_lambda_deploy_role_arn" {
  value = aws_iam_role.github_actions_lambda.arn
}
