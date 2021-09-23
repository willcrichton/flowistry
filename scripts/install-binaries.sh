set -e
cd $(mktemp -d)

REPO="willcrichton/flowistry"
TARGET=$(rustc -Vv | grep host | cut -d' ' -f2)
RELEASE="${RELEASE:-$(curl -s https://api.github.com/repos/${REPO}/releases/latest  | jq -r '.name')}"
RELEASE_URL="https://github.com/${REPO}/releases/download/${RELEASE}"

BINARY_PATH="${TARGET}.tar.gz"
BINARY_URL="${RELEASE_URL}/${BINARY_PATH}"
LOCAL_BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"

echo "Downloading release files from: ${BINARY_URL}"
if curl --location --output /dev/null --silent --head --fail ${BINARY_URL}; then
  curl --location --silent ${BINARY_URL} --output ${BINARY_PATH}
  tar -xf ${BINARY_PATH}
  rm ${BINARY_PATH}
  mkdir -p ${LOCAL_BIN_DIR}
  mv ./* ${LOCAL_BIN_DIR}  
  echo "Flowistry installed!"
else
  exit 1
fi