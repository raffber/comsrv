#!/bin/bash

set -euxfo pipefail

curdir=$(dirname "$0")
rootdir=$(realpath "$curdir"/..)

cd "$rootdir"/comsrv

user_id="$1"
group_id="$2"

groupadd -g $group_id docker
useradd -u $user_id -g $group_id docker 

chown -R $user_id:$group_id /cargo /home /rust
usermod -d /home docker

# sudo -E -u docker env PATH="$PATH" cargo build --release
sudo -E -u docker env PATH="$PATH" cargo xwin build --target x86_64-pc-windows-msvc --release