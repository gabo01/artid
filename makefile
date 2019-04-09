init:
	git config core.hooksPath .githooks

build:
	cargo build --all --verbose

test:
	cargo test --all --verbose
	cargo test --all --verbose -- --ignored

style:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings

# runs all the main ci commands locally. Theoretically, running this allows to avoid failure
# on the continuous integration build
ci:
	cargo build --all --verbose
	cargo test --all --verbose
	cargo test --all --verbose -- --ignored
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings