.DEFAULT_GOAL := help

# Override from the command line, e.g. make run ARGS='--config demo.toml'.
CARGO ?= cargo
UV ?= uv
NIX ?= nix
ARGS ?=
DOCS_ADDR ?= 127.0.0.1:8000
PREFIX ?= $(HOME)/.local
MANDIR ?= $(PREFIX)/share/man
DESTDIR ?=

.PHONY: help build release run install check test test-focus test-tmux fmt lint verify hooks \
	docs-setup docs-generate docs-check docs-build docs-serve man man-install \
	clean docs-clean dev nix-build nix-check nix-fmt

##@ Help
help: ## Show grouped targets and common overrides (default)
	@awk 'BEGIN { FS = ":.*## "; print "Usage: make <target> [VARIABLE=value]" } /^##@ / { printf "\n%s\n", substr($$0, 5) } /^[a-zA-Z0-9_-]+:.*## / { printf "  %-18s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@printf '\nOverrides: ARGS, DOCS_ADDR, PREFIX, MANDIR, DESTDIR, CARGO, UV, NIX\n'
	@printf 'Example:   make run ARGS="--config examples/demo.toml"\n'

##@ Nix toolchain
dev: ## Enter the locked Rust and documentation development shell
	$(NIX) develop

nix-build: ## Build the release package and manpage into result/
	$(NIX) build

nix-check: ## Build and test the package plus offline quality checks with Nix
	$(NIX) flake check

nix-fmt: ## Format the flake using the locked Nix formatter
	$(NIX) fmt

##@ Build and run
build: ## Build the debug binary in target/debug
	$(CARGO) build --locked

release: ## Build the optimized binary in target/release
	$(CARGO) build --locked --release

run: ## Run nysos; pass CLI options with ARGS='...'
	$(CARGO) run --locked -- $(ARGS)

install: ## Install the binary with Cargo (uses CARGO_HOME, not PREFIX)
	$(CARGO) install --locked --path .

##@ Quality and Git hooks
check: ## Type-check all Rust targets
	$(CARGO) check --locked --all-targets

test: ## Run unit and real PTY integration tests
	$(CARGO) test --locked

test-focus: build ## Verify Alt-arrow and Ghostty Option-arrow focus through a real PTY
	python3 scripts/test_focus.py

test-tmux: build ## Test prefixes, cues, mouse, and resize through a private tmux client
	python3 scripts/test_tmux.py

fmt: ## Format Rust source files
	$(CARGO) fmt --all

lint: ## Run Clippy with warnings treated as errors
	$(CARGO) clippy --locked --all-targets -- -D warnings

verify: ## Check formatting, lint, tests, demo config, and generated references
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --locked --all-targets -- -D warnings
	$(CARGO) test --locked
	$(CARGO) run --locked -- --config examples/demo.toml --check
	$(MAKE) docs-check

hooks: ## Activate the pre-commit hook for this Git repository
	git config core.hooksPath .githooks

##@ Documentation and manual
# Python tooling stays separate from Rust verification and the pre-commit hook.
docs-setup: ## Install locked Zensical dependencies into .venv (requires uv)
	$(UV) sync --locked

docs-generate: ## Regenerate CLI reference and manpage from src/cli.rs
	$(CARGO) run --locked --example generate_docs

docs-check: ## Fail when the checked-in CLI reference or manpage is stale
	$(CARGO) run --locked --example generate_docs -- --check

docs-build: docs-check ## Build the Zensical site into site/
	$(UV) run --locked zensical build --clean --strict

docs-serve: docs-check ## Preview docs with live reload at DOCS_ADDR
	$(UV) run --locked zensical serve --dev-addr $(DOCS_ADDR)

man: ## Read the checked-in nysos(1) manual
	man ./docs/man/nysos.1

man-install: ## Install nysos(1); supports PREFIX, MANDIR, and DESTDIR
	install -d "$(DESTDIR)$(MANDIR)/man1"
	install -m 644 docs/man/nysos.1 "$(DESTDIR)$(MANDIR)/man1/nysos.1"

##@ Cleanup
clean: ## Remove Rust build artifacts
	$(CARGO) clean

docs-clean: ## Remove generated site and Zensical cache; keep source and .venv
	rm -rf site .cache
