output "jwt_signing_key_secret_arn" {
  value = aws_secretsmanager_secret.jwt_signing_key.arn
}

output "jwt_refresh_pepper_secret_arn" {
  value = aws_secretsmanager_secret.jwt_refresh_pepper.arn
}

output "secret_arns" {
  value = [
    aws_secretsmanager_secret.jwt_signing_key.arn,
    aws_secretsmanager_secret.jwt_refresh_pepper.arn,
  ]
}
