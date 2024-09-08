#!/bin/bash

set -eufo pipefail

cd $(dirname "$0")
cd ..

version=$(grep -m 1 -oE '## \[.*?\]' CHANGELOG.md | sed -e 's/[# \[]//g' -e 's/\]//g')

arg=""
if [[ ! -z ${1+x} ]]; then
    arg=$1
fi  

if [[ $arg == "--no-pre-release" ]]; then
    echo $version
    exit 0
fi

on_release_tag=false
tag_to_head=$(git tag --points-at HEAD)
if [[ $tag_to_head =~ release.* ]]; then
    on_release_tag=true
fi

if [[ $on_release_tag == false ]]; then
    version="${version}-pre.$(date +%Y%m%d%H%M%S)"
fi

echo $version
