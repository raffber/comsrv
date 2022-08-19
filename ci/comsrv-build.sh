#!/bin/bash

set -euxfo pipefail

if ! [ -f /.dockerenv ]; then
    echo "This script must run in a docker container. Otherwise it screws up your environment.";
    exit 1;
fi

curdir=$(dirname "$0")
rootdir=$(realpath "$curdir"/..)

cd "$rootdir"/comsrv

user_id="$1"
group_id="$2"

groupadd -g $group_id docker
useradd -u $user_id -g $group_id docker 

chown -R $user_id:$group_id /cargo /home /rust
usermod -d /home docker

sudo -E -u docker env PATH="$PATH" cargo build --release
sudo -E -u docker env PATH="$PATH" cargo xwin build --target x86_64-pc-windows-msvc --release