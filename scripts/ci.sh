#! /bin/bash

if cargo build --verbose && cargo test --all --verbose; then
    echo "Build passed"

    if [ "$TRAVIS_RUST_VERSION" = "stable" ]; then
        echo "Style checks to be made"

        if cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings; then
            echo "Style checks passed"
        else
            exit 1
        fi
    else
        echo "Style checks won't be made"
    fi
else
    exit 1
fi
