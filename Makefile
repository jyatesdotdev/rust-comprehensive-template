.PHONY: help security audit deny geiger clippy-security semver-checks install-security-tools

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# --- Security scanning targets ---

security: audit deny geiger clippy-security semver-checks ## Run all security scans

audit: ## CVE scan via cargo-audit (RustSec advisory DB)
	cargo audit

deny: ## License, advisory, and ban checks via cargo-deny
	cargo deny check

geiger: ## Report unsafe code usage via cargo-geiger
	cargo geiger --all-features --all-targets

clippy-security: ## Run clippy with warnings promoted to errors
	cargo clippy --workspace --all-targets --all-features -- -D warnings

semver-checks: ## API compatibility check via cargo-semver-checks
	cargo semver-checks check-release

install-security-tools: ## Install all security scanning tools
	cargo install cargo-audit cargo-deny cargo-geiger cargo-semver-checks
