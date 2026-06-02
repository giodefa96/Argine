# Argine — developer entry points. Local security gate mirrors CI.
.DEFAULT_GOAL := help
.PHONY: help hooks security secrets backend-security frontend-security

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | \
		awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'

hooks: ## Install local git hooks (pre-commit + pre-push)
	pre-commit install --install-hooks
	pre-commit install --hook-type pre-push
	@echo "✅ git hooks installed"

security: secrets backend-security frontend-security ## Run the full local security gate (mirrors CI)
	@echo "✅ local security gate passed"

secrets: ## Scan the whole repo for committed secrets
	@if command -v gitleaks >/dev/null 2>&1; then \
		gitleaks detect --no-banner --redact; \
	else \
		echo "⚠️  gitleaks not installed — 'brew install gitleaks' (or run via pre-commit)"; \
	fi

backend-security: ## Rust supply-chain & advisory checks (cargo-deny)
	@if [ -f backend/Cargo.toml ]; then \
		cd backend && cargo deny check; \
	else \
		echo "ℹ️  no backend/Cargo.toml yet — skipping backend security"; \
	fi

frontend-security: ## Frontend dependency audit (npm audit)
	@if [ -f frontend/package.json ]; then \
		cd frontend && npm audit --audit-level=high; \
	else \
		echo "ℹ️  no frontend/package.json yet — skipping frontend security"; \
	fi
