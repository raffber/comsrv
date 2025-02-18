#!/bin/bash

set -euxfo pipefail

cd "$(dirname "$0")/.."

set -a
source .env
set +a

wheel=$(find dist -name "*.whl" | head -n 1)

cat <<EOF > ~/.pypirc
[distutils]
index-servers = custom-registry

[custom-registry]
repository = ${CUSTOM_REGISTRY_URL}
username = ${CUSTOM_REGISTRY_USERNAME}
password = ${CUSTOM_REGISTRY_PASSWORD}
EOF

./pw poetry run twine upload --repository custom-registry "$wheel" --verbose
