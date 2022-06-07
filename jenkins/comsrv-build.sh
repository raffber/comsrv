#!/bin/bash

set -euxfo pipefail

curdir=$(dirname "$0")
rootdir=$(realpath "$curdir"/..)

cd "$rootdir"/comsrv

user_id="$1"
group_id="$2"

groupadd -g $group_id docker
useradd -u $user_id -g $group_id docker 
# echo "docker:docker" | chpasswd
# adduser docker sudo

chown -R $user_id:$group_id /cargo /home /rust
usermod -d /home docker

# echo $HOME
# which cargo
# cargo xwin build --target x86_64-pc-windows-msvc --release
su - docker -c "cargo build"

# echo $PATH

# sudo -u docker -H echo $PATH
# sudo -u docker -H cargo build 