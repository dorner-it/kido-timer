.PHONY: setup \
	backend-dev backend-test backend-build backend-check \
	frontend-install frontend-dev frontend-build frontend-check \
	lint fmt \
	tf-init tf-fmt tf-validate tf-plan tf-apply \
	dynamodb-up dynamodb-down

setup:
	rustup target add aarch64-unknown-linux-musl
	@command -v cargo-lambda >/dev/null 2>&1 || cargo install cargo-lambda
	cd frontend && npm install

# ----- backend -----

backend-dev:
	cd backend && cargo lambda watch

backend-test:
	cd backend && cargo test

backend-build:
	cd backend && cargo lambda build --release --arm64 --output-format zip

backend-check:
	cd backend && cargo check --all-targets

# ----- frontend -----

frontend-install:
	cd frontend && npm install

frontend-dev:
	cd frontend && npm run dev

frontend-build:
	cd frontend && npm run build

frontend-check:
	cd frontend && npm run check

# ----- quality -----

lint:
	cd backend && cargo clippy --all-targets -- -D warnings

fmt:
	cd backend && cargo fmt
	cd infrastructure && terraform fmt -recursive

# ----- terraform -----

tf-init:
	cd infrastructure && terraform init

tf-fmt:
	cd infrastructure && terraform fmt -recursive -check

tf-validate:
	cd infrastructure && terraform validate

tf-plan:
	cd infrastructure && terraform plan

tf-apply:
	cd infrastructure && terraform apply

# ----- local services -----

dynamodb-up:
	docker compose up -d dynamodb-local

dynamodb-down:
	docker compose down
