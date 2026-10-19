# Infrastructure

Terraform for the KI-DO Timer Web backend: DynamoDB single table, AWS Cognito User Pool + Hosted UI, AWS Lambda (ARM64 / `provided.al2023`), API Gateway HTTP API, CloudWatch, and the two GitHub Actions OIDC roles.

State is shared with `../fms/`:

- bucket: `dorner-it-fms-terraform-state` (key: `ki-do-timer/terraform.tfstate`)
- state lock: S3-native lock file (`use_lockfile = true`, Terraform >= 1.10); the fms lock table `fms-terraform-locks` is no longer used

The GitHub OIDC provider is shared too — referenced via `data` rather than recreated.

## One-time bootstrap

The two GitHub Actions roles are declared in this stack, so the very first apply must be run locally with admin AWS credentials. After that GitHub Actions takes over.

```sh
cp terraform.tfvars.example terraform.tfvars.dev
# Fill in cognito_hosted_ui_domain_prefix and any prod URLs you have

terraform init
terraform plan -var-file=terraform.tfvars.dev
terraform apply -var-file=terraform.tfvars.dev
```

The first apply will:

- create the DynamoDB single table with GSI1, PITR, TTL, SSE
- create the Cognito User Pool, app client, and Hosted UI domain
- create the Lambda function (initially loaded with whatever zip is at `../backend/target/lambda/bootstrap/bootstrap.zip` — run `make backend-build` first)
- create the API Gateway HTTP API
- create the two GitHub Actions IAM roles (`<prefix>-github-actions`, `<prefix>-github-actions-terraform`)

After apply, set the role ARNs from the outputs as repo-level secrets in GitHub:

- `AWS_LAMBDA_DEPLOY_ROLE_ARN` ← `github_actions_lambda_deploy_role_arn`
- `AWS_TERRAFORM_ROLE_ARN`     ← `github_actions_terraform_role_arn`

CI takes over from there.

## Cognito users

Self-service signup is **disabled**. Create the 4 organizers manually:

```sh
aws cognito-idp admin-create-user \
  --user-pool-id "$(terraform output -raw cognito_user_pool_id)" \
  --username 'organizer@example.de' \
  --user-attributes Name=email,Value=organizer@example.de Name=name,Value='Andreas Dorner' \
  --desired-delivery-mediums EMAIL
```

Cognito emails the user a temporary password they're forced to change on first sign-in.

## Module layout

- `modules/dynamodb` — single table + GSI1, PITR, TTL, SSE, deletion protection
- `modules/cognito` — user pool, web client, hosted UI domain
- `modules/api` — Lambda + API Gateway HTTP API + IAM role + CloudWatch Log Group

The Lambda IAM role is least-privilege:

- DynamoDB CRUD on the single table and its GSIs
- `cognito-idp:DescribeUserPoolClient` (to fetch the client secret) and `cognito-idp:RevokeToken` (logout) — scoped to this user pool
- Standard `AWSLambdaBasicExecutionRole` for CloudWatch Logs

The Lambda's `filename` and `source_code_hash` are in `lifecycle.ignore_changes`, so subsequent Terraform applies do not overwrite the code that GitHub Actions deploys via `aws lambda update-function-code`.
