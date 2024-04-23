#!/bin/bash

set -eufo pipefail

cd $(dirname "$0")

version=$(./get-version.sh)

echo "Expected reference version: ${version}"

cd ..

changelog_version=$(grep -m 1 -oE '## \[.*?\]' CHANGELOG.md | sed -e 's/[# \[]//g' -e 's/\]//g')
client_version=$(grep -E -m 1 "^version" client/Cargo.toml | cut -d "=" -f 2 | sed 's/[" ]//g')
protocol_version=$(grep -E -m 1 "^version" protocol/Cargo.toml | cut -d "=" -f 2 | sed 's/[" ]//g')
py_version=$(grep -E -m 1 "^version" pyproject.toml | cut -d "=" -f 2 | sed 's/[" ]//g')

if [[ $version != $changelog_version ]]; then
    echo "Invalid version in changelog. Found: ${changelog_version}"
    exit 1
fi

if [[ $version != $client_version ]]; then
    echo "Invalid version in client. Found: ${client_version}"
    exit 1
fi

if [[ $version != $protocol_version ]]; then
    echo "Invalid version in protocol. Found: ${protocol_version}"
    exit 1
fi

if [[ $version != $py_version ]]; then
    echo "Invalid version in python package. Found: ${py_version}"
    exit 1
fi

echo "All version numbers are ok."
