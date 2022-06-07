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

(
cat <<'EOF'
#!/bin/bash

set -euxfo pipefail

export PATH="/cargo/bin:/rust/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

cd /data/comsrv

cargo build --release
cargo xwin build --target x86_64-pc-windows-msvc --release
EOF
) > /home/build.sh

chmod +x /home/build.sh

sudo -E -u docker /home/build.sh

