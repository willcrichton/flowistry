#!/bin/bash
set -e

cargo fmt
cargo clippy --workspace --all-features

pushd ide
npm run fmt
npm run lint
popd