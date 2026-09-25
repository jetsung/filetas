# Filetas 需求文档

基于 Rust + Axum 的高性能文件加速下载服务。

> 代码位置：`src/main.rs`（单文件实现）、`templates/index.html`（首页模板）

---

## 1. 服务配置与启动

### 1.1 命令行与环境变量配置

系统通过 clap 支持以下配置项（CLI 参数与环境变量等效）：

| 配置项   | 短参   | 长参             | 环境变量       | 默认值                     |
| -------- | ------ | ---------------- | -------------- | -------------------------- |
| 监听地址 | `-H`   | `--host`         | `HOST`         | `0.0.0.0`                  |
| 监听端口 | `-p`   | `--port`         | `PORT`         | `3000`                     |
| 页面标题 | `-t`   | `--title`        | `TITLE`        | `文件加速下载`             |
| 模板目录 | （无） | `--template-dir` | `TEMPLATE_DIR` | `templates`                |
| 上游 UA  | （无） | `--user-agent`   | `USER_AGENT`   | Chrome 134 Linux UA 字符串 |
| 上游代理 | （无） | `--proxy`        | `PROXY`        | （空，不使用代理）         |
| 详细日志 | `-v`   | `--verbose`      | （无）         | false                      |
| 静默日志 | `-q`   | `--quiet`        | （无）         | false                      |

- 环境变量配置端口：设置 `PORT=9000` 且不传 `--port` 时，服务监听 9000 端口。
- 不传任何参数时使用全部默认值：监听 `0.0.0.0:3000`，标题「文件加速下载」，模板目录 `templates`。

### 1.2 日志级别与初始化

日志级别按以下规则确定（`RUST_LOG` 环境变量可覆盖默认值）：

- `-v` / `--verbose`：`filetas=debug,tower_http=debug`
- `-q` / `--quiet`：`filetas=warn,tower_http=warn`
- 默认：`filetas=info,tower_http=info`

系统使用 tower-http `TraceLayer` 记录 HTTP 请求/响应 span。

- 以 `-v` 启动且未设置 `RUST_LOG` 时，日志级别为 debug。
- 设置 `RUST_LOG=error` 时，即使传 `-v`，实际日志过滤仍为 `error`。

### 1.3 服务启动与监听

启动时系统：

1. 解析并固定全局配置（OnceLock），打印版本号与配置摘要日志；
2. 校验模板文件 `<TEMPLATE_DIR>/index.html` 是否存在，缺失时记录 warn 并提示使用内置回退页；
3. 绑定 `host:port`，host 非法或绑定失败时打印 error 日志并以非零退出码退出；
4. 启动成功后打印监听地址与 Web 界面访问地址（host 为 `0.0.0.0` 时显示 `localhost`）。

- 端口被占用时：打印 error 日志并以非零退出码退出。
- 默认配置启动成功：日志显示 `Server listening on 0.0.0.0:3000` 及 `http://localhost:3000`。

---

## 2. 入口路由与 URL 解析

### 2.1 入口路径解析与请求路由

系统以 fallback 路由接收所有 HTTP 请求，按以下优先级解析：

1. 路径 `/` → 返回首页（见 4.1）；
2. 路径 `/favicon.ico` → 返回 favicon 响应（见 4.2）；
3. 路径 `/robots.txt` → 返回 robots 响应（见 4.2）；
4. 其余路径 → 视为待代理的目标 URL，进入代理流程。

- 请求 `/` 返回首页 HTML，不触发代理逻辑。
- `/favicon.ico`、`/robots.txt` 即使形式上可解析为域名，也优先走内置响应。
- 请求 `/https://example.com/file.zip?x=1` 时，取去除前导 `/` 后的路径作为目标 URL，并将原始查询串以 `?` 追加。

### 2.2 目标 URL 协议修复与补全

对提取出的目标字符串依次做规范化处理：

1. **URL 解码**（仅解码路径部分，单层解码、不递归；失败时使用原始值）——解码必须先于协议修复执行，且「先解码、后请求」保证等价性（见下）；
2. **修复协议变形**（某些客户端的转义结果）：
   - `http:/http://`、`https:/https://` → 还原为 `http://`、`https://`
   - `http:/`、`https:/`（单斜杠）→ 补全为 `http://`、`https://`
3. 处理后以 `http://` 或 `https://` 开头 → 直接作为完整目标 URL；
4. 否则若请求带 **Referer** → 以 Referer 的 `scheme://host` 为前缀拼接（`{scheme}://{host}/{path}`）；
5. 否则取首个 `/` 之前的部分，若其为有效域名（可解析且 host 含 `.`）→ 补全 `https://` 前缀；
6. 以上均不满足 → 返回 `400 Bad Request`，响应体 `Invalid URL`。

**「先解码、后请求」等价性约束**：

- 百分号编码形式的目标 URL（如 `/https%3A%2F%2Fcdn.example.com%2Fa.JPG`）在解码后发起的上游请求，必须与未编码形式（`/https://cdn.example.com/a.JPG`）逐字符一致（上游 URL、路径相同，上游返回相同状态码与响应）；
- 查询串原样透传、不重复解码（如 `?name=a%2Bb` 上游收到的仍是 `?name=a%2Bb`）；
- 仅解码一层，不递归解码二次编码形式（如 `%253A` 只解码为 `%3A`）。

示例：

| 输入目标                        | 条件                                      | 代理目标                                    |
| ------------------------------- | ----------------------------------------- | ------------------------------------------- |
| `https:/github.com/a/b`         | —                                         | `https://github.com/a/b`                    |
| `https:/https://example.com/f`  | —                                         | `https://example.com/f`                     |
| `https%3A%2F%2Fexample.com%2Ff` | —                                         | `https://example.com/f`（与未编码形式一致） |
| `example.com/file.zip`          | 无 Referer                                | `https://example.com/file.zip`              |
| `a/b`                           | Referer 为 `https://ref.example.com/page` | `https://ref.example.com/a/b`               |
| `noturl`                        | 无协议、首段非域名、无 Referer            | 400 `Invalid URL`                           |

---

## 3. 反向代理转发

### 3.1 方法白名单与 OPTIONS 处理

- 仅允许 `GET`、`HEAD`、`POST` 发起代理转发；其他方法返回 `405 Method Not Allowed`（响应体 `Method Not Allowed`），不发起任何上游请求。
- `OPTIONS` 请求不进入代理转发，由系统直接处理：
  - 同时带 `Origin`、`Access-Control-Request-Method`、`Access-Control-Request-Headers` 时视为 **CORS 预检**：返回 200 与统一 CORS 头，`Access-Control-Allow-Headers` 回显请求中的 `Access-Control-Request-Headers` 值；
  - 否则视为**普通 OPTIONS**：返回 200 与统一 CORS 头，并附 `Allow: GET, HEAD, POST, OPTIONS`。

统一 CORS 头：

```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET,HEAD,POST,OPTIONS
Access-Control-Max-Age: 86400
Access-Control-Expose-Headers: *
```

### 3.2 上游请求构造

- 使用独立配置的 reqwest 客户端：固定 UA（`--user-agent`）、连接超时 15s、总超时 60s、**禁用自动重定向**（由服务端代答）；
- 配置了上游代理（`--proxy` / `PROXY`，支持 `http://`、`https://`、`socks5://`）时，所有上游请求（含 Git 端点专用客户端）SHALL 经该代理转发；代理地址非法时记录 warn 并忽略代理（直连）；
- 复制客户端请求头，但过滤：
  - 逐跳首部：`connection`、`keep-alive`、`proxy-authenticate`、`proxy-authorization`、`te`、`trailer`、`transfer-encoding`、`upgrade`；
  - `Content-Length`；
- `HOST` 头改写为最终目标 URL 的 host（如客户端请求 `proxy.local`、目标 `https://example.com/f`，上游 HOST 为 `example.com`）。

### 3.3 重定向手动跟随

上游返回 3xx 且带 `Location` 头时，系统手动跟随重定向：

- `Location` 为绝对 URL → 直接使用；
- 为相对路径 → 基于原目标 URL 解析（`Url::join`），解析失败时退化为字符串拼接（原 URL 去尾斜杠 + Location）；
- 301/302/303 → 以 `GET` 方法、空请求头跟随；
- 307/308 → 保留原 HTTP 方法与原请求头跟随（含 `Content-Type`、`Authorization`），使带请求体的 POST（如 git `git-upload-pack`）在重定向后仍语义完整；
- 客户端禁用自动重定向由服务端代答（Git 端点由专用客户端的重定向策略代答）。

| 场景                                                             | 结果                                  |
| ---------------------------------------------------------------- | ------------------------------------- |
| 302 + `Location: https://cdn.example.com/f.zip`                  | 以 GET 请求该地址并向客户端返回其响应 |
| 目标 `https://a.example.com/d/x` 返回 302 + `Location: /files/y` | 跟随 `https://a.example.com/files/y`  |

### 3.4 流式响应与响应头处理

- 上游响应体以**流式**（`bytes_stream`）透传给客户端，状态码透传；
- 复制响应头（存在时）：`Content-Type`、`Content-Length`、`Content-Disposition`、`Content-Encoding`；
- 附加统一 CORS 头；
- 删除安全相关头：`Set-Cookie`、`Content-Security-Policy`、`Content-Security-Policy-Report-Only`、`Clear-Site-Data`；
- 上游请求失败（超时/拒绝）→ 返回 `500`，响应体 `Failed to send request`。

### 3.5 GitHub 加速规则

在发起代理请求前，对目标 URL 应用以下 GitHub 匹配规则：

| 模式             | 匹配格式                                                             | 处理                              |
| ---------------- | -------------------------------------------------------------------- | --------------------------------- |
| releases/archive | `github.com/<owner>/<repo>/(releases\|archive)/...`                  | 直接转发                          |
| blob/raw         | `github.com/<owner>/<repo>/(blob\|raw)/...`                          | 将 `/blob/` 替换为 `/raw/` 后转发 |
| info/git-        | `github.com/<owner>/<repo>/(info\|git-)...`                          | 直接转发                          |
| raw 内容         | `raw.(githubusercontent\|github).com/<owner>/<repo>/<branch>/<path>` | 直接转发                          |
| gist             | `gist.(githubusercontent\|github).com/<user>/<gist-id>/<ref>`        | 直接转发                          |
| tags             | `github.com/<owner>/<repo>/tags...`                                  | 直接转发                          |

（以上均允许带或不带 `https?://` 前缀。）

- 示例：`https://github.com/o/r/blob/main/f.txt` → 代理 `https://github.com/o/r/raw/main/f.txt`；`raw` URL 保持不变；非 GitHub URL 按原 URL 代理。

### 3.6 GitHub API Token 注入

- 目标 URL 的 host 为 `api.github.com`，且环境变量 `GITHUB_TOKEN` 存在且非空时，向上游请求添加 `Authorization: Bearer <GITHUB_TOKEN>`；
- 其他目标不注入该头（即使 `GITHUB_TOKEN` 已设置）。

### 3.7 Git 智能 HTTP 代理

系统识别 Git 智能 HTTP 协议端点并按 git 语义代理，支持 `git clone http://<proxy>/<git-url>`：

- **端点识别**（URL 解码后判断，编码形式同样命中）：
  - GET `*/info/refs?service=git-upload-pack|git-receive-pack|git-upload-archive`（协议握手）；
  - POST `*/git-upload-pack|git-receive-pack|git-upload-archive`（数据传输）；
- **双向流式透传**：POST 请求体转发给上游，响应体流式返回，`Content-Type`（如 `application/x-git-upload-pack-request`）原样透传；
- **专用客户端**：Git 端点使用独立上游客户端——连接超时 15 秒、**无总超时**（大仓库长时流式传输不被截断）；自定义重定向策略（最多 5 次，307/308 保留方法与请求头、301/302/303 转 GET）；
- **非 git 端点行为不变**：仍使用原客户端（60 秒总超时、手动重定向跟随）。

示例：

| 命令                                                                           | 结果                 |
| ------------------------------------------------------------------------------ | -------------------- |
| `git clone http://localhost:3000/https://atomgit.com/jetsung/sh.git`           | 克隆完整成功         |
| `git clone http://localhost:3000/https%3A%2F%2Fatomgit.com%2Fjetsung%2Fsh.git` | 编码形式同样克隆成功 |

---

## 4. Web 界面与站点元数据

### 4.1 首页模板渲染

- 优先读取 `<TEMPLATE_DIR>/index.html` 模板，将其中 `{{ title }}` 占位符替换为配置的标题（TITLE）；
- 模板读取失败时回退到内置回退页（同样做 `{{ title }}` 替换），并记录 warn 日志，服务不报错。

### 4.2 favicon 与 robots.txt

- `/favicon.ico` → 返回 `204 No Content`；
- `/robots.txt` → 返回 `200`，`Content-Type: text/plain`，响应体：

```
User-agent:*
Disallow:/
```

---

## 5. 部署与运行

- 开发运行：`cargo run`；
- 生产构建：`cargo build --release` 后运行 `./target/release/filetas`；
- TLS 后端使用 rustls（纯 Rust 实现，reqwest `rustls` 特性，另启用 `socks` 支持 socks5 上游代理），无需安装 OpenSSL / `libssl-dev`，仅需 Rust 工具链（https://rustup.rs）；
- 提供 Docker 镜像，支持 amd64/arm64 多平台部署（见 `docker/` 目录）；
- 可通过 `GITHUB_TOKEN` 环境变量进行 GitHub API 身份验证，避免速率限制。
