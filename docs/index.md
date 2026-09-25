---
icon: lucide/rocket
---

# Filetas

基于 Rust + Axum 的高性能文件加速下载服务 (High-performance file acceleration download service)

## 功能特性

- **高性能**: 基于 Rust 和 Axum 框架，提供极致性能。
- **GitHub 加速**: 自动识别并加速 GitHub 文件下载。
- **GitHub API 支持**: 支持通过 `GITHUB_TOKEN` 环境变量进行身份验证，避免 API 速率限制。
- **Git 智能 HTTP 代理**: 支持 `git clone` 经代理拉取代码仓库。
- **智能重定向**: 自动处理 HTTP 重定向和 URL 转换。
- **CORS 支持**: 完整的跨域资源共享支持。
- **多种访问方式**: 支持带协议头（`https://...`）、不带协议头（`github.com/...`）、单斜杠或百分号编码形式的代理请求。
- **现代界面**: 蓝色渐变设计的现代化 Web 界面。
- **详细日志**: 结构化日志记录，支持多级别日志输出。
- **灵活配置**: 支持命令行参数和环境变量配置。
- **纯 Rust TLS**: 使用 rustls，无需安装 OpenSSL。
- **容器化**: 提供 Docker 镜像，支持 amd64/arm64 多平台部署。

## 快速开始

### 先决条件

TLS 后端使用 rustls（纯 Rust 实现），无需安装 OpenSSL，仅需 Rust 工具链：<https://rustup.rs>

### 安装运行

1. **下载源码**

```bash
git clone https://atomgit.com/jetsung/filetas.git
cd filetas
```

2. **开发环境运行**

```bash
cargo run
```

3. **生产环境构建**

```bash
cargo build --release
./target/release/filetas
```

## 使用方法

### 命令行参数

```bash
filetas [OPTIONS]

选项:
  -H, --host <HOST>              监听地址 [env: HOST=] [默认: 0.0.0.0]
  -p, --port <PORT>              监听端口 [env: PORT=] [默认: 3000]
  -t, --title <TITLE>            页面标题 [env: TITLE=] [默认: 文件加速下载]
      --template-dir <DIR>       模板目录 [env: TEMPLATE_DIR=] [默认: templates]
      --user-agent <UA>          上游 User-Agent [env: USER_AGENT=]
  -v, --verbose                  详细日志（debug）
  -q, --quiet                    静默日志（warn）
  -h, --help                     帮助
  -V, --version                  版本
```

### 文件加速下载

浏览器访问首页输入下载链接，或直接拼接：

```text
https://<proxy>/<url>

# 带协议头
https://localhost:3000/https://github.com/user/repo/releases/download/v1.0/file.zip

# 不带协议头
https://localhost:3000/github.com/user/repo/releases/download/v1.0/file.zip

# 百分号编码形式
https://localhost:3000/https%3A%2F%2Fgithub.com%2Fuser%2Frepo%2Freleases%2Fdownload%2Fv1.0%2Ffile.zip
```

### Git clone 加速

```bash
# 原始 URL
git clone http://localhost:3000/https://atomgit.com/jetsung/sh.git

# 编码 URL（等价）
git clone http://localhost:3000/https%3A%2F%2Fatomgit.com%2Fjetsung%2Fsh.git
```

### 环境变量

| 变量           | 说明            | 默认值          |
| -------------- | --------------- | --------------- |
| `HOST`         | 监听地址        | `0.0.0.0`       |
| `PORT`         | 监听端口        | `3000`          |
| `TITLE`        | 页面标题        | `文件加速下载`  |
| `TEMPLATE_DIR` | 模板目录        | `templates`     |
| `USER_AGENT`   | 上游 UA         | Chrome Linux UA |
| `GITHUB_TOKEN` | GitHub API 令牌 | （空）          |
| `RUST_LOG`     | 日志过滤        | 按 `-v`/`-q`    |

## 部署

### Docker

```bash
docker run -d -p 3000:3000 --name filetas ghcr.io/jetsung/filetas:latest
```

支持 `linux/amd64` 与 `linux/arm64` 多平台镜像，并同步发布至阿里云 ACR、华为云 HCR、腾讯云 TCR。

### 二进制发布

通过 tag 触发 CI，产出 Linux（x86_64/aarch64/loongarch64 的 gnu 与 musl）、macOS（x86_64/aarch64）、Windows（x86_64/aarch64）二进制包，并发布到 crates.io。

## 许可证

本项目采用 Apache-2.0 许可证 - 查看 [LICENSE](https://atomgit.com/jetsung/filetas/blob/main/LICENSE) 了解详情。

## 仓库镜像

[MyCode](https://git.jetsung.com/jetsung/filetas) ● [AtomGit](https://atomgit.com/jetsung/filetas) ● [GitHub](https://github.com/jetsung/filetas)
