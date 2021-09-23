#!/bin/bash

set -e
cd $(mktemp -d)

REPO="willcrichton/flowistry"
TARGET=$(rustc -Vv | grep host | cut -d' ' -f2)
LATEST=$(curl -s https://api.github.com/repos/${REPO}/releases/latest  | jq -r '.name')
RELEASE_URL="https://github.com/${REPO}/releases/download/${LATEST}"

BINARY_PATH="${TARGET}.tar.gz"
BINARY_URL="${RELEASE_URL}/${BINARY_PATH}"
LOCAL_BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"

if wget -q --spider ${BINARY_URL}; then
  wget -q ${BINARY_URL}
  tar -xf ${BINARY_PATH}
  rm ${BINARY_PATH}
  mkdir -p ${LOCAL_BIN_DIR}
  mv ./* ${LOCAL_BIN_DIR}  
else
  exit 1
fi