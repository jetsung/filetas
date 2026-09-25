# Filetas 构建与运行脚本
# 用法: just <recipe>  （不带参数运行 `just` 查看全部命令）

# Current version read from Cargo.toml
current_version := `sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1`

default:
    @just --list

# 开发构建（debug）
build:
    cargo build

# 生产构建（release）
build-release:
    cargo build --release

# 运行开发服务（先构建再启动）
run:
    cargo run

# 运行 release 服务（需先 just build-release）
run-release:
    ./target/release/filetas

# 代码检查（编译检查 + clippy + 格式检查）
check:
    cargo check
    cargo clippy -- -D warnings
    cargo fmt --check

# 构建 deb 包（需先有 target/release/filetas；交叉编译时见 release.yml）
build-deb: build-release
    cargo deb --no-build --no-strip
    @echo "输出: target/debian/*.deb"

# 构建 rpm 包（需先有 target/release/filetas）
build-rpm: build-release
    cargo generate-rpm
    @echo "输出: target/generate-rpm/*.rpm"

# 同时构建 deb + rpm
build-packages: build-deb build-rpm

# 本地构建 Docker 镜像（filetas:local，参照 docker/docker-bake.hcl）
build-docker:
    docker buildx bake --file docker/docker-bake.hcl default

# 构建多平台 Docker 镜像（amd64/arm64，需 buildx 多平台支持）
build-docker-multi:
    docker buildx bake --file docker/docker-bake.hcl default --push

# 清理构建产物
clean:
    cargo clean
    rm -rf target/debian target/generate-rpm

# ============================================================================
# Release
# ============================================================================

# 显示当前版本号
version:
    @echo "当前版本: v{{ current_version }}"

# 更新版本号（Cargo.toml + Cargo.lock），如: just bump-version 0.5.0
bump-version VERSION=current_version:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "{{ VERSION }}" == "{{ current_version }}" ]]; then
        echo "版本号未变化（当前 {{ current_version }}）。用法: just bump-version <新版本号>"
        exit 1
    fi
    if ! grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$' <<<"{{ VERSION }}"; then
        echo "无效版本号: {{ VERSION }}（须为 X.Y.Z 格式）"
        exit 1
    fi
    sed -i "s/^version = \".*\"/version = \"{{ VERSION }}\"/" Cargo.toml
    cargo update -p filetas --precise "{{ VERSION }}" >/dev/null 2>&1 || cargo check -q
    echo "版本号已更新: {{ current_version }} -> {{ VERSION }}"
    echo "涉及文件: Cargo.toml / Cargo.lock"

# 创建新版本发布（更新版本号 + 提交 + 签名 tag），如: just release 0.5.0
release VERSION:
    #!/usr/bin/env bash
    set -euo pipefail
    just bump-version {{ VERSION }}
    git add Cargo.toml Cargo.lock
    git commit -m "chore: 版本号升级至 {{ VERSION }}"
    git tag -s "v{{ VERSION }}" -m "v{{ VERSION }}"
    echo ""
    echo "已创建签名 tag v{{ VERSION }}，推送以触发发布:"
    echo "  git push origin main v{{ VERSION }}"
