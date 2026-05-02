output "user_pool_id" {
  value = aws_cognito_user_pool.this.id
}

output "user_pool_arn" {
  value = aws_cognito_user_pool.this.arn
}

output "client_id" {
  value = aws_cognito_user_pool_client.web.id
}

output "hosted_ui_domain" {
  description = "Fully qualified Cognito hosted UI domain (e.g. auth-ki-do-it.auth.eu-central-1.amazoncognito.com)."
  value       = "${aws_cognito_user_pool_domain.this.domain}.auth.${data.aws_region.current.name}.amazoncognito.com"
}

output "issuer_url" {
  description = "OIDC issuer URL used to fetch JWKS."
  value       = "https://cognito-idp.${data.aws_region.current.name}.amazonaws.com/${aws_cognito_user_pool.this.id}"
}

data "aws_region" "current" {}
