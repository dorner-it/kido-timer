data "aws_iam_policy_document" "lambda_assume" {
  statement {
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "lambda" {
  name               = "${var.prefix}-lambda-exec"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume.json
  tags               = var.tags
}

resource "aws_iam_role_policy_attachment" "basic_exec" {
  role       = aws_iam_role.lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

data "aws_iam_policy_document" "ddb" {
  statement {
    actions = [
      "dynamodb:GetItem",
      "dynamodb:PutItem",
      "dynamodb:UpdateItem",
      "dynamodb:DeleteItem",
      "dynamodb:Query",
      "dynamodb:BatchGetItem",
      "dynamodb:BatchWriteItem",
      "dynamodb:ConditionCheckItem",
      "dynamodb:TransactGetItems",
      "dynamodb:TransactWriteItems",
    ]
    resources = [
      var.table_arn,
      "${var.table_arn}/index/*",
    ]
  }
}

resource "aws_iam_role_policy" "ddb" {
  name   = "${var.prefix}-ddb"
  role   = aws_iam_role.lambda.id
  policy = data.aws_iam_policy_document.ddb.json
}

# Cognito: read the user-pool client (for the client secret used in the
# Authorization Code exchange). Scoped to this user pool.
data "aws_iam_policy_document" "cognito" {
  statement {
    actions = [
      "cognito-idp:DescribeUserPoolClient",
      "cognito-idp:RevokeToken",
    ]
    resources = [var.cognito_user_pool_arn]
  }
}

resource "aws_iam_role_policy" "cognito" {
  name   = "${var.prefix}-cognito"
  role   = aws_iam_role.lambda.id
  policy = data.aws_iam_policy_document.cognito.json
}

resource "aws_cloudwatch_log_group" "lambda" {
  name              = "/aws/lambda/${var.function_name}"
  retention_in_days = var.log_retention_days
  tags              = var.tags
}

resource "aws_lambda_function" "this" {
  function_name = var.function_name
  role          = aws_iam_role.lambda.arn

  runtime       = "provided.al2023"
  handler       = "bootstrap"
  architectures = ["arm64"]

  filename         = var.lambda_zip_path
  source_code_hash = fileexists(var.lambda_zip_path) ? filebase64sha256(var.lambda_zip_path) : null

  memory_size = var.memory_size_mb
  timeout     = var.timeout_seconds

  environment {
    variables = {
      TABLE_NAME           = var.table_name
      LOG_LEVEL            = var.log_level
      ENV                  = var.env
      COGNITO_USER_POOL_ID = var.cognito_user_pool_id
      COGNITO_CLIENT_ID    = var.cognito_client_id
      COGNITO_DOMAIN       = var.cognito_hosted_ui_domain
      COGNITO_REDIRECT_URI = var.cognito_redirect_uri
      FRONTEND_BASE_URL    = var.frontend_base_url
    }
  }

  # GitHub Actions deploys code via aws lambda update-function-code, so
  # subsequent Terraform runs must not revert the code to the locally-built
  # zip.
  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }

  depends_on = [
    aws_cloudwatch_log_group.lambda,
    aws_iam_role_policy_attachment.basic_exec,
  ]

  tags = var.tags
}

resource "aws_apigatewayv2_api" "this" {
  name          = var.function_name
  protocol_type = "HTTP"

  dynamic "cors_configuration" {
    for_each = length(var.cors_allowed_origins) > 0 ? [1] : []
    content {
      allow_origins     = var.cors_allowed_origins
      allow_methods     = ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
      allow_headers     = ["Authorization", "Content-Type", "X-API-Key"]
      allow_credentials = true
      max_age           = 600
    }
  }

  tags = var.tags
}

resource "aws_apigatewayv2_integration" "lambda" {
  api_id                 = aws_apigatewayv2_api.this.id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.this.invoke_arn
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_route" "any" {
  api_id    = aws_apigatewayv2_api.this.id
  route_key = "ANY /{proxy+}"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

resource "aws_apigatewayv2_stage" "default" {
  api_id      = aws_apigatewayv2_api.this.id
  name        = "$default"
  auto_deploy = true
  tags        = var.tags
}

resource "aws_lambda_permission" "apigw" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.this.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_api.this.execution_arn}/*/*"
}
