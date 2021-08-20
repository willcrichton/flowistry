#!/bin/bash

set -e
cd $(mktemp -d)

REPO="willcrichton/flowistry"
TARGET=$(rustc -Vv | grep host | cut -d' ' -f2)
LATEST=$(curl -s https://api.github.com/repos/${REPO}/releases/latest  | jq -r '.name')
RELEASE_URL="https://github.com/${REPO}/releases/download/${LATEST}"

BINARY_PATH="${TARGET}.tar.gz"
BINARY_URL="${RELEASE_URL}/${BINARY_PATH}"
LOCAL_BIN_DIR="$HOME/.local/bin"

EXTENSION_PATH="vscode-extension.tgz"
EXTENSION_URL="${RELEASE_URL}/${EXTENSION_PATH}"
VSC_EXT_DIR="${HOME}/.vscode/extensions"

if wget -q --spider ${BINARY_URL}; then
  wget -q ${BINARY_URL}
  tar -xf ${BINARY_PATH}
  rm ${BINARY_PATH}
  mkdir -p ${LOCAL_BIN_DIR}
  mv ./* ${LOCAL_BIN_DIR}  

  wget -q ${EXTENSION_URL}
  tar -xf ${EXTENSION_PATH}
  mkdir -p ${VSC_EXT_DIR}
  if [ -d ${VSC_EXT_DIR}/flowistry ]; then
    echo "Error: Flowistry is already installed at path \"${VSC_EXT_DIR}/flowistry\"."
    echo "  If you want to install, please delete that directory and re-run this script."
    exit 1
  fi  
  mv package ${VSC_EXT_DIR}/flowistry
else
  echo "Error: your detected platform of \"${TARGET}\" is not an available Flowistry build."
  echo "  If the target is incorrect, please file an issue. If you want support for your"
  echo "  platform, please submit a pull request."
fi

echo "Installation succeeded! Try opening a Rust project in VSCode (or restart if it's already open)."