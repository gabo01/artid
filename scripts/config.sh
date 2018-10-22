#! /bin/bash

if [ "$TRAVIS_RUST_VERSION" = "stable" ]; then
    rustup component add rustfmt-preview
    rustup component add clippy-preview
    echo "Added components"
else
    echo "Components not added"
fi