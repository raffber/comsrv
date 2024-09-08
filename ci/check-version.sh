#!/bin/bash

set -eufo pipefail

cd $(dirname "$0")

version=$(./get-version.sh --no-pre-release)

echo "Expected reference version: ${version}"

cd ..

changelog_version=$(grep -m 1 -oE '## \[.*?\]' CHANGELOG.md | sed -e 's/[# \[]//g' -e 's/\]//g')
client_version=$(grep -E -m 1 "^version" client/Cargo.toml | cut -d "=" -f 2 | sed 's/[" ]//g')
protocol_version=$(grep -E -m 1 "^version" protocol/Cargo.toml | cut -d "=" -f 2 | sed 's/[" ]//g')
py_version=$(grep -E -m 1 "^version" pyproject.toml | cut -d "=" -f 2 | sed 's/[" ]//g')
dart_version=$(grep -E -m 1 "^version" dart/comsrv/pubspec.yaml | cut -d ":" -f 2 | sed 's/[" ]//g')

fail=false

if [[ $version != $changelog_version ]]; then
    echo "Invalid version in changelog. Found: ${changelog_version}"
    fail=true
fi

if [[ $version != $client_version ]]; then
    echo "Invalid version in client. Found: ${client_version}"
    fail=true
fi

if [[ $version != $protocol_version ]]; then
    echo "Invalid version in protocol. Found: ${protocol_version}"
    fail=true
fi

if [[ $version != $py_version ]]; then
    echo "Invalid version in python package. Found: ${py_version}"
    fail=true
fi

if [[ $version != $dart_version ]]; then
    echo "Invalid version in dart package. Found: ${dart_version}"
    fail=true
fi

if [[ $fail == true ]]; then
    exit 1
fi

echo "All version numbers are ok."
