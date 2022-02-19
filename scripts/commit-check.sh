#!/bin/bash
set -e

cargo fmt
cargo clippy --workspace

pushd ide
npm run fmt
npm run lint
popd