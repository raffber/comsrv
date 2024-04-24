#!/bin/bash

set -euo pipefail

cd $(dirname "$0")/..

version=$(./ci/get-version.sh)

replace_version_toml() {

python_script=$(cat <<EOF
import re

with open("$1", "r") as f:
    content = f.read()

content = re.sub(r'version = "[\d\.]+"', 'version = "${version}"', content, count=1)

with open("$1", "w") as f:
    f.write(content)
EOF
)

python -c "$python_script"

}

replace_version_yaml() {

python_script=$(cat <<EOF
import re

with open("$1", "r") as f:
    content = f.read()

content = re.sub(r'version: [\d\.]+', 'version: ${version}', content, count=1)

with open("$1", "w") as f:
    f.write(content)
EOF
)

python -c "$python_script"

}


replace_version_toml client/Cargo.toml
replace_version_toml protocol/Cargo.toml
replace_version_toml comsrv/Cargo.toml
replace_version_toml pyproject.toml
replace_version_yaml dart/comsrv/pubspec.yaml

./ci/check-version.sh
