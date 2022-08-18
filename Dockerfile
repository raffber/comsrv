FROM lukemathwalker/cargo-chef:latest-rust-1.63-bullseye as chef
WORKDIR /workspace

ENV KEYRINGS /usr/local/share/keyrings

RUN set -eux; \
    mkdir -p $KEYRINGS; \
    apt-get update && apt-get install -y gpg curl; \
    # clang/lld/llvm
    curl --fail https://apt.llvm.org/llvm-snapshot.gpg.key | gpg --dearmor > $KEYRINGS/llvm.gpg; \
    # wine
    curl --fail https://dl.winehq.org/wine-builds/winehq.key | gpg --dearmor > $KEYRINGS/winehq.gpg; \
    echo "deb [signed-by=$KEYRINGS/llvm.gpg] http://apt.llvm.org/bullseye/ llvm-toolchain-bullseye-13 main" > /etc/apt/sources.list.d/llvm.list; \
    echo "deb [signed-by=$KEYRINGS/winehq.gpg] https://dl.winehq.org/wine-builds/debian/ bullseye main" > /etc/apt/sources.list.d/winehq.list;

RUN set -eux; \
    dpkg --add-architecture i386; \
    # Skipping all of the "recommended" cruft reduces total images size by ~300MiB
    apt-get update && apt-get install --no-install-recommends -y \
    clang-13 \
    # llvm-ar
    llvm-13 \
    lld-13 \
    # get a recent wine so we can run tests
    winehq-staging \
    # Unpack xwin
    tar; \
    # ensure that clang/clang++ are callable directly
    ln -s clang-13 /usr/bin/clang && ln -s clang /usr/bin/clang++ && ln -s lld-13 /usr/bin/ld.lld; \
    # We also need to setup symlinks ourselves for the MSVC shims because they aren't in the debian packages
    ln -s clang-13 /usr/bin/clang-cl && ln -s llvm-ar-13 /usr/bin/llvm-lib && ln -s lld-link-13 /usr/bin/lld-link; \
    # Verify the symlinks are correct
    clang++ -v; \
    ld.lld -v; \
    # Doesn't have an actual -v/--version flag, but it still exits with 0
    llvm-lib -v; \
    clang-cl -v; \
    lld-link --version; \
    # Use clang instead of gcc when compiling binaries targeting the host (eg proc macros, build files)
    update-alternatives --install /usr/bin/cc cc /usr/bin/clang 100; \
    update-alternatives --install /usr/bin/c++ c++ /usr/bin/clang++ 100; \
    apt-get remove -y --auto-remove; \
    rm -rf /var/lib/apt/lists/*;

RUN rustup target add x86_64-pc-windows-msvc && \
    cargo install xwin 

RUN mkdir /cache && \
    xwin --accept-license --cache-dir /cache/xwin-temp splat --output /xwin && \
    rm -rf /cache/xwin-temp

FROM lukemathwalker/cargo-chef:latest-rust-1.63-bullseye as planner
WORKDIR /workspace
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder

RUN apt-get update && \
    apt-get install --yes build-essential libudev-dev udev pkg-config libusb-1.0-0-dev curl sudo 

COPY --from=planner /workspace/recipe.json recipe.json
RUN cargo chef cook --release  --recipe-path recipe.json && mv target /cache/linux-release-cache

ENV CC_x86_64_pc_windows_msvc="clang-cl" \
    CXX_x86_64_pc_windows_msvc="clang-cl" \
    AR_x86_64_pc_windows_msvc="llvm-lib" \
    CL_FLAGS="-Wno-unused-command-line-argument -fuse-ld=lld-link /imsvc/xwin/crt/include /imsvc/xwin/sdk/include/ucrt /imsvc/xwin/sdk/include/um /imsvc/xwin/sdk/include/shared" \
    CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER="lld-link" \
    CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS="-Lnative=/xwin/crt/lib/x86_64 -Lnative=/xwin/sdk/lib/um/x86_64 -Lnative=/xwin/sdk/lib/ucrt/x86_64"


ENV CFLAGS_x86_64_pc_windows_msvc="$CL_FLAGS" CXXFLAGS_x86_64_pc_windows_msvc="$CL_FLAGS"

RUN cargo chef cook --release --target  x86_64-pc-windows-msvc --recipe-path recipe.json && mv target /cache/windows-release-cache
