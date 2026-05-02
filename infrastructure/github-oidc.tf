# The GitHub Actions OIDC provider is shared with ../fms/ — already provisioned
# in the same AWS account. We reference it via a data source rather than
# creating a duplicate (which would conflict).
data "aws_iam_openid_connect_provider" "github" {
  url = "https://token.actions.githubusercontent.com"
}

# ---------------------------------------------------------------------------
# Role 1: Lambda code deploy.
# Used by .github/workflows/deploy-lambda.yaml on push to main.
# ---------------------------------------------------------------------------

data "aws_iam_policy_document" "github_actions_lambda_assume" {
  statement {
    actions = ["sts:AssumeRoleWithWebIdentity"]
    principals {
      type        = "Federated"
      identifiers = [data.aws_iam_openid_connect_provider.github.arn]
    }
    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:aud"
      values   = ["sts.amazonaws.com"]
    }
    condition {
      test     = "StringLike"
      variable = "token.actions.githubusercontent.com:sub"
      values   = ["repo:${var.github_repo}:ref:refs/heads/main"]
    }
  }
}

resource "aws_iam_role" "github_actions_lambda" {
  name               = "${local.prefix}-github-actions"
  assume_role_policy = data.aws_iam_policy_document.github_actions_lambda_assume.json
  tags               = local.default_tags
}

data "aws_iam_policy_document" "github_actions_lambda" {
  statement {
    actions = [
      "lambda:UpdateFunctionCode",
      "lambda:GetFunction",
    ]
    resources = ["arn:aws:lambda:${var.region}:*:function:${local.prefix}"]
  }
}

resource "aws_iam_role_policy" "github_actions_lambda" {
  name   = "${local.prefix}-lambda-deploy"
  role   = aws_iam_role.github_actions_lambda.id
  policy = data.aws_iam_policy_document.github_actions_lambda.json
}

# ---------------------------------------------------------------------------
# Role 2: Terraform plan/apply.
# Used by .github/workflows/terraform.yaml on push to main.
# ---------------------------------------------------------------------------

data "aws_iam_policy_document" "github_actions_terraform_assume" {
  statement {
    actions = ["sts:AssumeRoleWithWebIdentity"]
    principals {
      type        = "Federated"
      identifiers = [data.aws_iam_openid_connect_provider.github.arn]
    }
    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:aud"
      values   = ["sts.amazonaws.com"]
    }
    condition {
      test     = "StringLike"
      variable = "token.actions.githubusercontent.com:sub"
      values   = ["repo:${var.github_repo}:ref:refs/heads/main"]
    }
  }
}

resource "aws_iam_role" "github_actions_terraform" {
  name               = "${local.prefix}-github-actions-terraform"
  assume_role_policy = data.aws_iam_policy_document.github_actions_terraform_assume.json
  tags               = local.default_tags
}

data "aws_iam_policy_document" "github_actions_terraform" {
  statement {
    sid = "ManageOwnedResources"
    actions = [
      "lambda:*",
      "apigateway:*",
      "execute-api:*",
      "cognito-idp:*",
      "dynamodb:*",
      "logs:CreateLogGroup",
      "logs:DeleteLogGroup",
      "logs:DescribeLogGroups",
      "logs:ListTagsForResource",
      "logs:PutRetentionPolicy",
      "logs:TagResource",
      "logs:UntagResource",
      "iam:GetRole",
      "iam:GetRolePolicy",
      "iam:CreateRole",
      "iam:DeleteRole",
      "iam:PutRolePolicy",
      "iam:DeleteRolePolicy",
      "iam:ListRolePolicies",
      "iam:ListAttachedRolePolicies",
      "iam:AttachRolePolicy",
      "iam:DetachRolePolicy",
      "iam:UpdateAssumeRolePolicy",
      "iam:PassRole",
      "iam:TagRole",
      "iam:UntagRole",
      "iam:GetOpenIDConnectProvider",
    ]
    resources = ["*"]
  }

  statement {
    sid = "TerraformStateBucket"
    actions = [
      "s3:GetObject",
      "s3:PutObject",
      "s3:ListBucket",
    ]
    resources = [
      "arn:aws:s3:::dorner-it-fms-terraform-state",
      "arn:aws:s3:::dorner-it-fms-terraform-state/*",
    ]
  }

  statement {
    sid = "TerraformStateLock"
    actions = [
      "dynamodb:GetItem",
      "dynamodb:PutItem",
      "dynamodb:DeleteItem",
    ]
    resources = ["arn:aws:dynamodb:${var.region}:*:table/fms-terraform-locks"]
  }
}

resource "aws_iam_role_policy" "github_actions_terraform" {
  name   = "${local.prefix}-terraform-manage"
  role   = aws_iam_role.github_actions_terraform.id
  policy = data.aws_iam_policy_document.github_actions_terraform.json
}
