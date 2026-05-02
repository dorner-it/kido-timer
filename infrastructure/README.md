# Infrastructure

Terraform for the KI-DO Timer Web backend: DynamoDB single table, AWS Lambda (ARM64 / `provided.al2023`), API Gateway HTTP API, Secrets Manager, CloudWatch.

## Prerequisites

- Terraform >= 1.6
- AWS credentials with permission to create the resources above
- Lambda binary built: `make backend-build` from the repo root produces `backend/target/lambda/bootstrap/bootstrap.zip`
- Remote-state bucket and DynamoDB lock table provisioned out-of-band (one-time per AWS account)

## First-time setup

1. Copy the tfvars template:

   ```sh
   cp terraform.tfvars.example terraform.tfvars.dev
   ```

   Fill in `env`, `region`, and (once a Vercel domain is allocated) `cors_allowed_origins`.

2. Create a backend config file `backend-dev.hcl`:

   ```hcl
   bucket         = "your-tfstate-bucket"
   key            = "ki-do-timer/dev/terraform.tfstate"
   region         = "eu-central-1"
   dynamodb_table = "your-tflock-table"
   encrypt        = true
   ```

3. Uncomment the `terraform { backend "s3" {} }` block in `backend.tf`, then initialize:

   ```sh
   terraform init -backend-config=backend-dev.hcl
   ```

4. Plan:

   ```sh
   terraform plan -var-file=terraform.tfvars.dev
   ```

5. Apply (only after review):

   ```sh
   terraform apply -var-file=terraform.tfvars.dev
   ```

## Module layout

- `modules/dynamodb` — single table + GSI1, PITR, TTL, server-side encryption
- `modules/secrets` — JWT signing key and refresh-token pepper, generated as random strings
- `modules/api` — Lambda + API Gateway HTTP API + IAM role + CloudWatch Log Group

The IAM role for the Lambda is least-privilege: DynamoDB CRUD scoped to the single table and its GSIs, plus `secretsmanager:GetSecretValue` scoped to the two secret ARNs.
