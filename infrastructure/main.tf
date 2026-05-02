locals {
  prefix = "${var.project}-${var.env}"

  default_tags = merge({
    Project     = var.project
    Environment = var.env
    ManagedBy   = "terraform"
  }, var.tags)
}

module "dynamodb" {
  source     = "./modules/dynamodb"
  table_name = local.prefix
  tags       = local.default_tags
}

module "cognito" {
  source = "./modules/cognito"

  user_pool_name          = local.prefix
  hosted_ui_domain_prefix = var.cognito_hosted_ui_domain_prefix
  callback_urls           = var.cognito_callback_urls
  logout_urls             = var.cognito_logout_urls
  tags                    = local.default_tags
}

module "api" {
  source = "./modules/api"

  function_name            = local.prefix
  prefix                   = local.prefix
  env                      = var.env
  lambda_zip_path          = var.lambda_zip_path
  table_name               = module.dynamodb.table_name
  table_arn                = module.dynamodb.table_arn
  cognito_user_pool_id     = module.cognito.user_pool_id
  cognito_user_pool_arn    = module.cognito.user_pool_arn
  cognito_client_id        = module.cognito.client_id
  cognito_hosted_ui_domain = module.cognito.hosted_ui_domain
  cognito_redirect_uri     = var.cognito_redirect_uri
  frontend_base_url        = var.frontend_base_url
  cors_allowed_origins     = var.cors_allowed_origins
  log_level                = var.log_level
  log_retention_days       = var.log_retention_days
  tags                     = local.default_tags
}
