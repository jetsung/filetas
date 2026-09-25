# Filetas

基于 Rust + Axum 的高性能文件加速下载服务 (High-performance file acceleration download service)

## 功能特性

- **高性能**: 基于 Rust 和 Axum 框架，提供极致性能。
- **GitHub 加速**: 自动识别并加速 GitHub 文件下载。
- **GitHub API 支持**: 支持通过 `GITHUB_TOKEN` 环境变量进行身份验证，避免 API 速率限制。
- **Git clone 加速**: 支持 Git 智能 HTTP 协议代理，`git clone` 经代理拉取仓库。
- **上游代理**: 支持 `--proxy` 配置 http/https/socks5 上游代理。
- **智能重定向**: 自动处理 HTTP 重定向和 URL 转换（307/308 保留方法与请求头）。
- **CORS 支持**: 完整的跨域资源共享支持。
- **多种访问方式**: 支持带协议头（`https://...`）、不带协议头（`github.com/...`）、单斜杠或百分号编码形式的代理请求。
- **纯 Rust TLS**: 使用 rustls，无需安装 OpenSSL。
- **现代界面**: 蓝色渐变设计的现代化 Web 界面。
- **详细日志**: 结构化日志记录，支持多级别日志输出。
- **灵活配置**: 支持命令行参数和环境变量配置。
- **deb/rpm 打包**: 内置 cargo-deb / cargo-generate-rpm 配置，CI 自动发布。
- **容器化**: 提供 Docker 镜像，支持 amd64/arm64 多平台部署。

## 快速开始

### 先决条件

TLS 后端使用 rustls（纯 Rust 实现），无需安装 OpenSSL。

- Rust 工具链（含 cargo）：https://rustup.rs

### 安装运行

1. **下载源码**

```bash
git clone https://github.com/jetsung/filetas.git
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
  -H, --host <HOST>              服务器监听地址 [默认: 0.0.0.0] [环境变量: HOST]
  -p, --port <PORT>              服务器端口 [默认: 3000] [环境变量: PORT]
  -t, --title <TITLE>            页面标题 [默认: 文件加速下载] [环境变量: TITLE]
      --template-dir <DIR>       模板目录路径 [默认: templates] [环境变量: TEMPLATE_DIR]
      --user-agent <USER_AGENT>  请求用户代理 [环境变量: USER_AGENT]
      --proxy <PROXY>       上游代理地址 (http/https/socks5) [环境变量: PROXY]
  -v, --verbose                  启用详细日志 (等同于 RUST_LOG=debug)
  -q, --quiet                    启用安静模式 (等同于 RUST_LOG=warn)
  -h, --help                     显示帮助信息
  -V, --version                  显示版本信息
```

### 使用示例

```bash
# 基本使用
filetas

# 自定义端口和主机
filetas --host 127.0.0.1 --port 3000

# 自定义页面标题
filetas --title "我的文件服务器"

# 启用详细日志
filetas --verbose

# 使用环境变量
HOST=0.0.0.0 PORT=8080 TITLE="File Server" GITHUB_TOKEN=your_token filetas

# 组合使用
RUST_LOG=debug filetas --port 8080 --title "开发服务器"
```

### 环境变量

| 变量名         | 描述                                 | 默认值                         |
| -------------- | ------------------------------------ | ------------------------------ |
| `HOST`         | 服务器监听地址                       | `0.0.0.0`                      |
| `PORT`         | 服务器端口                           | `3000`                         |
| `TITLE`        | 页面标题                             | `文件加速下载`                 |
| `TEMPLATE_DIR` | 模板目录路径                         | `templates`                    |
| `USER_AGENT`   | 请求用户代理                         | `Mozilla/5.0 ...`              |
| `PROXY`        | 上游代理地址（http/https/socks5）    | （空，不使用代理）             |
| `GITHUB_TOKEN` | GitHub 个人访问令牌（用于 API 加速） | (无)                           |
| `RUST_LOG`     | 日志级别                             | `filetas=info,tower_http=info` |

### 日志配置

```bash
# 默认日志级别 (INFO)
filetas

# 详细调试日志
filetas --verbose
# 或
RUST_LOG=debug filetas

# 只显示警告和错误
filetas --quiet
# 或
RUST_LOG=warn filetas

# 自定义日志级别
RUST_LOG=filetas=trace,tower_http=debug filetas
```

## 支持的 URL 格式

### GitHub 文件加速

- **Releases**: `https://github.com/user/repo/releases/download/v1.0.0/file.zip`
- **Archive**: `https://github.com/user/repo/archive/refs/heads/main.zip`
- **Raw 文件**: `https://github.com/user/repo/raw/main/file.txt`
- **Blob 文件**: `https://github.com/user/repo/blob/main/file.txt` (自动转换为 raw)
- **Gist**: `https://gist.github.com/user/gist-id/raw/file.txt`
- **Tags**: `https://github.com/user/repo/tags`

### 多种请求方式示例

服务支持非常灵活的 URL 格式，会自动识别并补全：

- `http://localhost:3000/https://github.com/user/repo/archive/main.zip` (完整 URL)
- `http://localhost:3000/github.com/user/repo/archive/main.zip` (自动补全 https)
- `http://localhost:3000/https:/github.com/user/repo/archive/main.zip` (修正单斜杠)
- `http://localhost:3000/https%3A%2F%2Fgithub.com%2Fuser%2Frepo%2Farchive%2Fmain.zip` (百分号编码，与未编码形式等价)

### Git clone 加速

```bash
# 原始 URL
git clone http://localhost:3000/https://atomgit.com/jetsung/sh.git

# 编码 URL（等价）
git clone http://localhost:3000/https%3A%2F%2Fatomgit.com%2Fjetsung%2Fsh.git
```

支持 Git 智能 HTTP 协议（`info/refs` 握手与 `git-upload-pack` 等数据端点），大仓库长时传输无总超时限制。

### 通用文件下载

- 任何 HTTP/HTTPS 文件 URL
- 自动处理重定向
- 支持大文件流式传输

## Web 界面使用

1. 访问 `http://localhost:3000`
2. 在输入框中粘贴文件 URL
3. 点击下载按钮或按回车键
4. 文件将通过加速服务下载

## Docker 部署

### 使用预构建镜像

#### 可用镜像仓库

| 镜像仓库                  | 镜像地址                                                    | 说明              |
| ------------------------- | ----------------------------------------------------------- | ----------------- |
| Docker Hub                | `jetsung/filetas:latest`                                    | 官方镜像仓库      |
| GitHub Container Registry | `ghcr.io/jetsung/filetas:latest`                            | GitHub 容器注册表 |
| 阿里云容器镜像服务        | `registry.cn-guangzhou.aliyuncs.com/jetsung/filetas:latest` | 阿里云镜像        |
| 腾讯云容器镜像服务        | `sgccr.ccs.tencentyun.com/jetsung/filetas:latest`           | 腾讯云镜像        |

#### 运行示例

```bash
# Docker Hub
docker run -p 3000:3000 -d jetsung/filetas:latest

# GitHub Registry
docker run -p 3000:3000 -d ghcr.io/jetsung/filetas:latest

# 阿里云镜像（国内用户推荐）
docker run -p 3000:3000 -d registry.cn-guangzhou.aliyuncs.com/jetsung/filetas:latest

# 腾讯云镜像
docker run -p 3000:3000 -d sgccr.ccs.tencentyun.com/jetsung/filetas:latest

# 使用 GITHUB_TOKEN
docker run -p 3000:3000 -e GITHUB_TOKEN=your_token -d jetsung/filetas:latest
```

### Docker Compose

```yaml
services:
  filetas:
    image: jetsung/filetas:latest
    container_name: filetas
    restart: unless-stopped
    ports:
      - "3000:3000"
    environment:
      - HOST=0.0.0.0
      - PORT=3000
      - TITLE=文件加速下载
      - GITHUB_TOKEN=your_token_here
      - RUST_LOG=filetas=info
    volumes:
      - ./templates:/app/templates # 可选：自定义模板
```

### 构建自定义镜像

```bash
# 构建镜像
docker build -f docker/Dockerfile -t my-filetas .

# 运行
docker run -p 3000:3000 -d my-filetas
```

## API 使用

### 直接下载

```bash
# 通过服务下载文件
curl -L "http://localhost:3000/https://example.com/file.zip" -o file.zip

# GitHub 文件加速
curl -L "http://localhost:3000/https://github.com/user/repo/releases/download/v1.0.0/file.zip" -o file.zip
```

### CORS 支持

服务支持跨域请求，可以在前端 JavaScript 中直接使用：

```javascript
// 获取文件
fetch("http://localhost:3000/https://example.com/file.json")
  .then((response) => response.json())
  .then((data) => console.log(data));

// 下载文件
const downloadUrl =
  "http://localhost:3000/" + encodeURIComponent("https://example.com/file.zip");
window.open(downloadUrl);
```

## 开发

### 项目结构

```
filetas/
├── docker/
│   ├── Dockerfile              # Docker 构建文件
│   └── docker-bake.hcl         # Docker Bake 构建配置
├── src/
│   └── main.rs                 # 主程序
├── templates/
│   └── index.html              # Web 界面模板
├── Cargo.toml                  # 项目配置
├── Cargo.lock                  # 依赖锁定文件
└── README.md
```

### 本地开发

```bash
# 克隆项目
git clone https://github.com/jetsung/filetas.git
cd filetas

# 使用 justfile（推荐）
just run          # 开发运行
just build        # 开发构建
just build-release  # 生产构建
just build-deb    # 构建 deb 包
just build-rpm    # 构建 rpm 包
just build-docker # 本地 Docker 镜像
just check        # 编译检查 + clippy + 格式检查

# 或直接使用 cargo
cargo run

# 开启详细日志的开发模式
RUST_LOG=debug cargo run -- --verbose

# 运行测试
cargo test

# 代码格式化
cargo fmt

# 代码检查
cargo clippy
```

### Git 钩子（prek）

项目使用 [prek](https://github.com/j178/prek) 管理提交钩子，`prek run` 会自动修复格式（rustfmt / taplo / prettier）：

```bash
prek install      # 安装钩子
prek run --all-files  # 手动运行（自动修复格式）
```

### 项目结构

```
filetas/
├── docker/
│   ├── Dockerfile              # Docker 构建文件
│   └── docker-bake.hcl         # Docker Bake 构建配置
├── docs/                       # 文档站点（zensical）
├── src/
│   └── main.rs                 # 主程序
├── templates/
│   └── index.html              # Web 界面模板
├── justfile                    # 构建与运行脚本
├── prek.toml                   # Git 钩子配置
├── Cargo.toml                  # 项目配置（含 deb/rpm 打包元数据）
├── Cargo.lock                  # 依赖锁定文件
└── README.md
```

## 性能优化

- 使用 Rust 的零成本抽象和内存安全特性
- 基于 Tokio 异步运行时，支持高并发
- 流式传输大文件，减少内存占用
- 智能重定向处理，减少不必要的请求
- 结构化日志记录，便于性能分析

## 故障排除

### 常见问题

1. **端口被占用**
   ```bash
   filetas --port 8080
   ```
2. **模板文件未找到**
   ```bash
   filetas --template-dir /path/to/templates
   ```
3. **SSL/TLS 错误**
   项目使用纯 Rust 的 rustls，无需系统 OpenSSL；若遇到证书问题，检查系统根证书是否齐全。

## 贡献

欢迎提交 Issue 和 Pull Request！

## 许可证

本项目采用 Apache-2.0 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 仓库镜像

[MyCode](https://git.jetsung.com/jetsung/filetas) ● [AtomGit](https://atomgit.com/jetsung/filetas) ● [GitHub](https://github.com/jetsung/filetas)
