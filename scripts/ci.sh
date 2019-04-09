#! /bin/bash

cargo build --all --verbose
build=$?

cargo test --all --verbose
tests=$?

cargo test --all --verbose -- --ignored
ignored=$?

if [ "$build" = "0" ] && [ $tests = "0" ] && [ "$ignored" = "0" ]; then
    echo "Build passed"

    if [ "$TRAVIS_RUST_VERSION" = "stable" ] && [ "$TRAVIS_TAG" = "" ]; then
        echo "Style checks to be made"

        cargo fmt --all -- --check
        fmt=$?

        cargo clippy --all-targets --all-features -- -D warnings
        clippy=$?

        if [ "$fmt" = "0" ] && [ "$clippy" = "0" ]; then
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
