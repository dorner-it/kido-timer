resource "aws_secretsmanager_secret" "jwt_signing_key" {
  name        = "${var.prefix}/jwt-signing-key"
  description = "HS256 signing key for organizer JWTs."
  tags        = var.tags
}

resource "random_password" "jwt_signing_key" {
  length           = 64
  override_special = "-_"
}

resource "aws_secretsmanager_secret_version" "jwt_signing_key" {
  secret_id     = aws_secretsmanager_secret.jwt_signing_key.id
  secret_string = random_password.jwt_signing_key.result
}

resource "aws_secretsmanager_secret" "jwt_refresh_pepper" {
  name        = "${var.prefix}/jwt-refresh-pepper"
  description = "HMAC pepper for hashing stored refresh tokens."
  tags        = var.tags
}

resource "random_password" "jwt_refresh_pepper" {
  length           = 64
  override_special = "-_"
}

resource "aws_secretsmanager_secret_version" "jwt_refresh_pepper" {
  secret_id     = aws_secretsmanager_secret.jwt_refresh_pepper.id
  secret_string = random_password.jwt_refresh_pepper.result
}
