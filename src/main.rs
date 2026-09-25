use std::{net::SocketAddr, time::Instant};

use axum::{
    Router,
    body::Body,
    extract::RawQuery,
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header},
    response::{Html, IntoResponse, Response},
};
use clap::Parser;
use regex::Regex;
use tower::ServiceBuilder;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

/// Filetas - 文件加速下载服务
#[derive(Parser, Debug)]
#[command(name = "filetas")]
#[command(version = "0.3.1")]
#[command(about = "A file acceleration download service built with Rust and Axum")]
#[command(long_about = "
Filetas is a high-performance file acceleration download service that provides:
- GitHub file acceleration download
- General file proxy download
- CORS support for cross-origin requests
- Automatic URL processing and redirection
- Modern web interface with blue gradient design

The service supports various GitHub URL formats and automatically converts them
to the appropriate download URLs. It also provides a web interface for easy
file downloading.

Examples:
  filetas --host 127.0.0.1 --port 3000
  filetas --title \"My File Server\" --template-dir ./templates
  RUST_LOG=debug filetas --port 8080
")]
struct Args {
    /// Server host address
    #[arg(short = 'H', long, env = "HOST", default_value = "0.0.0.0")]
    host: String,

    /// Server port
    #[arg(short, long, env = "PORT", default_value = "3000")]
    port: u16,

    /// Page title
    #[arg(short, long, env = "TITLE", default_value = "文件加速下载")]
    title: String,

    /// Template directory path
    #[arg(long, env = "TEMPLATE_DIR", default_value = "templates")]
    template_dir: String,

    /// User agent string for requests
    #[arg(
        long,
        env = "USER_AGENT",
        default_value = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"
    )]
    user_agent: String,

    /// Enable verbose logging (equivalent to RUST_LOG=debug)
    #[arg(short, long)]
    verbose: bool,

    /// Enable quiet mode (equivalent to RUST_LOG=warn)
    #[arg(short, long)]
    quiet: bool,

    /// Upstream proxy URL (http://, https:// or socks5://)
    #[arg(long, env = "PROXY")]
    proxy: Option<String>,
}

// GitHub URL 匹配模式
struct RegexPatterns {
    releases: Regex,
    blob_raw: Regex,
    info_git: Regex,
    raw_content: Regex,
    gist: Regex,
    tags: Regex,
}

impl RegexPatterns {
    fn new() -> Self {
        Self {
            releases: Regex::new(r"^(?:https?://)?github\.com/.+?/.+?/(?:releases|archive)/.*$")
                .unwrap(),
            blob_raw: Regex::new(r"^(?:https?://)?github\.com/.+?/.+?/(?:blob|raw)/.*$").unwrap(),
            info_git: Regex::new(r"^(?:https?://)?github\.com/.+?/.+?/(?:info|git-).*$").unwrap(),
            raw_content: Regex::new(
                r"^(?:https?://)?raw\.(?:githubusercontent|github)\.com/.+?/.+?/.+?/.+$",
            )
            .unwrap(),
            gist: Regex::new(r"^(?:https?://)?gist\.(?:githubusercontent|github)\.com/.+?/.+?/.+$")
                .unwrap(),
            tags: Regex::new(r"^(?:https?://)?github\.com/.+?/.+?/tags.*$").unwrap(),
        }
    }
}

// 添加 CORS 头
fn add_cors_headers(headers: &mut HeaderMap) {
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        "GET,HEAD,POST,OPTIONS".parse().unwrap(),
    );
    headers.insert(header::ACCESS_CONTROL_MAX_AGE, "86400".parse().unwrap());
    headers.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, "*".parse().unwrap());
}

// 判断是否为有效域名
fn is_domain(url: &str) -> bool {
    if let Ok(parsed_url) = Url::parse(&format!("https://{}", url)) {
        if let Some(host) = parsed_url.host_str() {
            return host.contains('.');
        }
    }
    false
}

// 判断是否为 Git 智能 HTTP 端点（info/refs 握手或 upload-pack/receive-pack/upload-archive 数据端点）
fn is_git_endpoint(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let path = parsed.path();
    let git_services = ["git-upload-pack", "git-receive-pack", "git-upload-archive"];

    if path.ends_with("/info/refs") {
        // 握手端点须带 service=git-* 查询参数
        return parsed
            .query_pairs()
            .any(|(k, v)| k == "service" && git_services.contains(&v.as_ref()));
    }
    git_services
        .iter()
        .any(|svc| path.ends_with(&format!("/{}", svc)))
}

// 处理 OPTIONS 请求
async fn handle_options(headers: HeaderMap) -> impl IntoResponse {
    debug!("Handling OPTIONS request");
    let mut response_headers = HeaderMap::new();
    add_cors_headers(&mut response_headers);

    if headers.contains_key(header::ORIGIN)
        && headers.contains_key(header::ACCESS_CONTROL_REQUEST_METHOD)
        && headers.contains_key(header::ACCESS_CONTROL_REQUEST_HEADERS)
    {
        // Handle CORS preflight requests
        debug!("Handling CORS preflight request");
        if let Some(request_headers) = headers.get(header::ACCESS_CONTROL_REQUEST_HEADERS) {
            response_headers.insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                request_headers.clone(),
            );
        }
    } else {
        // Handle standard OPTIONS request
        debug!("Handling standard OPTIONS request");
        response_headers.insert(header::ALLOW, "GET, HEAD, POST, OPTIONS".parse().unwrap());
    }

    (StatusCode::OK, response_headers, "")
}

// 流式传输响应体
async fn stream_response(resp: reqwest::Response) -> impl IntoResponse {
    let status = resp.status();
    let url = resp.url().clone();
    let resp_headers = resp.headers().clone();

    info!("Streaming response: {} {}", status, url);

    // 记录响应头信息
    if let Some(content_type) = resp_headers.get(header::CONTENT_TYPE) {
        debug!("Content-Type: {:?}", content_type);
    }
    if let Some(content_length) = resp_headers.get(header::CONTENT_LENGTH) {
        debug!("Content-Length: {:?}", content_length);
    }

    let stream = resp.bytes_stream();
    let body = Body::from_stream(stream);
    let mut response = Response::new(body);
    let headers = response.headers_mut();

    // 复制重要的响应头
    if let Some(content_type) = resp_headers.get(header::CONTENT_TYPE) {
        headers.insert(header::CONTENT_TYPE, content_type.clone());
    }
    if let Some(content_length) = resp_headers.get(header::CONTENT_LENGTH) {
        headers.insert(header::CONTENT_LENGTH, content_length.clone());
    }
    if let Some(content_disposition) = resp_headers.get(header::CONTENT_DISPOSITION) {
        headers.insert(header::CONTENT_DISPOSITION, content_disposition.clone());
    }
    if let Some(content_encoding) = resp_headers.get(header::CONTENT_ENCODING) {
        headers.insert(header::CONTENT_ENCODING, content_encoding.clone());
    }

    // 添加 CORS 头
    add_cors_headers(headers);

    // 删除安全相关头
    headers.remove(header::SET_COOKIE);
    headers.remove(header::CONTENT_SECURITY_POLICY);
    headers.remove(header::CONTENT_SECURITY_POLICY_REPORT_ONLY);
    headers.remove("clear-site-data");

    *response.status_mut() = status;
    response
}

use std::sync::OnceLock;

// 全局配置
static CONFIG: OnceLock<Args> = OnceLock::new();
static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static GIT_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_config() -> &'static Args {
    CONFIG.get().unwrap()
}

fn get_client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        let config = get_config();
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(&config.user_agent)
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(60));
        builder = apply_proxy(builder);
        builder.build().unwrap()
    })
}

fn get_git_client() -> &'static reqwest::Client {
    GIT_CLIENT.get_or_init(|| {
        let config = get_config();
        // Git 端点专用客户端：无总超时（大仓库长时流式传输）；自定义重定向策略仅限制次数，
        // 方法语义由 reqwest 内建处理（307/308 保留原方法与请求头，301/302/303 转 GET）
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .user_agent(&config.user_agent)
            .connect_timeout(std::time::Duration::from_secs(15));
        builder = apply_proxy(builder);
        builder.build().unwrap()
    })
}

// 为客户端应用 --proxy / PROXY 上游代理（支持 http://、https://、socks5://）
fn apply_proxy(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    match get_config().proxy.as_deref() {
        Some(url) if !url.is_empty() => match reqwest::Proxy::all(url) {
            Ok(proxy) => {
                info!("Using upstream proxy: {}", url);
                builder.proxy(proxy)
            }
            Err(e) => {
                warn!("Invalid proxy URL '{}': {}, ignoring proxy", url, e);
                builder
            }
        },
        _ => builder,
    }
}

// 需要过滤的逐跳首部（hop-by-hop headers）
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

// 内置回退页面
const FALLBACK_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ title }}</title>
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; display: flex; justify-content: center; align-items: center; min-height: 100vh; background: #1a90ff; }
        .box { text-align: center; width: 90%; max-width: 600px; }
        h1 { color: #fff; font-size: 1.8em; margin-bottom: 20px; }
        .row { display: flex; height: 48px; }
        input { flex: 1; padding: 0 16px; font-size: 16px; border: none; border-radius: 8px 0 0 8px; outline: none; }
        button { padding: 0 24px; background: #0066ff; border: none; border-radius: 0 8px 8px 0; color: #fff; font-size: 16px; cursor: pointer; }
        button:hover { background: #0052cc; }
    </style>
</head>
<body>
<div class="box">
    <h1>文件加速下载</h1>
    <div class="row">
        <input type="text" id="url" placeholder="请输入下载链接">
        <button onclick="jump()">下载</button>
    </div>
</div>
<script>
function jump() { var u = document.getElementById('url').value; if (u) { var base = location.href.replace(/\/?$/, '/'); window.open(base + encodeURIComponent(u)); } }
document.getElementById('url').addEventListener('keypress', function(e) { if (e.key === 'Enter') jump(); });
</script>
</body>
</html>"#;

// 处理 / 请求
async fn handler_index() -> Html<String> {
    debug!("Serving index page");
    let config = get_config();
    let template_path = format!("{}/index.html", config.template_dir);
    let html_content = std::fs::read_to_string(&template_path).unwrap_or_else(|e| {
        warn!(
            "Failed to load {}: {}, using built-in fallback page",
            template_path, e
        );
        FALLBACK_HTML.to_string()
    });

    Html(html_content.replace("{{ title }}", &config.title))
}

// 处理 favicon.ico 请求
async fn handler_favicon() -> impl IntoResponse {
    (StatusCode::NO_CONTENT, "")
}

// 处理 robots.txt 请求
async fn handler_robots() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
    (StatusCode::OK, headers, "User-agent:*\nDisallow:/")
}

// 代理请求处理
async fn proxy_request(
    url: &str,
    method: reqwest::Method,
    headers: HeaderMap,
    body: Option<reqwest::Body>,
) -> Result<reqwest::Response, reqwest::Error> {
    let start_time = Instant::now();
    debug!("Sending {} request to: {}", method, url);

    // Git 智能 HTTP 端点使用专用客户端（无总超时；自动跟随重定向且 307/308 保留方法与请求头）
    let is_git = is_git_endpoint(url);
    let client = if is_git {
        get_git_client()
    } else {
        get_client()
    };
    let mut request = client.request(method.clone(), url).headers(headers);
    if let Some(body) = body {
        request = request.body(body);
    }
    let response = request.send().await?;

    let elapsed = start_time.elapsed();
    let status = response.status();

    info!(
        "Request completed: {} {} - {} ({:.2}ms)",
        method,
        url,
        status,
        elapsed.as_millis()
    );

    // 处理重定向（非 git 客户端手动跟随；git 客户端由自定义重定向策略自动处理）
    if !is_git && response.status().is_redirection() {
        if let Some(location) = response.headers().get(header::LOCATION) {
            if let Ok(location_str) = location.to_str() {
                let redirect_url = if location_str.starts_with("http://")
                    || location_str.starts_with("https://")
                {
                    location_str.to_string()
                } else {
                    // 处理相对 URL：基于原始 URL 解析
                    if let Ok(base) = Url::parse(url) {
                        if let Ok(resolved) = base.join(location_str) {
                            resolved.to_string()
                        } else {
                            format!("{}{}", url.trim_end_matches('/'), location_str)
                        }
                    } else {
                        format!("{}{}", url.trim_end_matches('/'), location_str)
                    }
                };
                info!("Following redirect to: {}", redirect_url);
                return Box::pin(proxy_request(
                    &redirect_url,
                    reqwest::Method::GET,
                    HeaderMap::new(),
                    None,
                ))
                .await;
            }
        }
    }

    Ok(response)
}

// HTTP 请求处理
async fn http_request(
    req_url: &str,
    method: Method,
    request_headers: HeaderMap,
    body: Option<reqwest::Body>,
) -> Result<reqwest::Response, reqwest::Error> {
    let patterns = RegexPatterns::new();

    let final_url = if patterns.releases.is_match(req_url)
        || patterns.gist.is_match(req_url)
        || patterns.tags.is_match(req_url)
        || patterns.info_git.is_match(req_url)
        || patterns.raw_content.is_match(req_url)
    {
        debug!("Matched GitHub pattern for: {}", req_url);
        req_url.to_string()
    } else if patterns.blob_raw.is_match(req_url) {
        let transformed_url = req_url.replace("/blob/", "/raw/");
        debug!(
            "Transformed GitHub blob URL: {} -> {}",
            req_url, transformed_url
        );
        transformed_url
    } else {
        debug!("Using original URL: {}", req_url);
        req_url.to_string()
    };

    let req_method = reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap();
    let mut headers = HeaderMap::new();

    // 复制请求头，过滤掉逐跳首部和 Content-Length
    for (key, value) in request_headers.iter() {
        let key_str = key.as_str().to_lowercase();
        if HOP_BY_HOP.contains(&key_str.as_str()) {
            continue;
        }
        if key == header::HOST {
            if let Ok(parsed_url) = Url::parse(&final_url) {
                if let Some(host) = parsed_url.host_str() {
                    headers.insert(key, HeaderValue::from_str(host).unwrap());
                    debug!("Updated HOST header to: {}", host);
                }
            }
        } else if key != header::CONTENT_LENGTH {
            headers.insert(key, value.clone());
        }
    }

    // 处理 GitHub API Token
    if let Ok(parsed_url) = Url::parse(&final_url) {
        if parsed_url.host_str() == Some("api.github.com") {
            if let Ok(token) = std::env::var("GITHUB_TOKEN") {
                if !token.is_empty() {
                    let auth_value = format!("Bearer {}", token);
                    if let Ok(header_value) = HeaderValue::from_str(&auth_value) {
                        headers.insert(header::AUTHORIZATION, header_value);
                        debug!("Added Authorization header for api.github.com");
                    }
                }
            }
        }
    }

    proxy_request(&final_url, req_method, headers, body).await
}

// 执行请求
async fn do_request(
    req_url: &str,
    method: Method,
    headers: HeaderMap,
    body: Option<reqwest::Body>,
) -> impl IntoResponse {
    info!("Processing request: {} {}", method, req_url);

    if method == Method::OPTIONS {
        return handle_options(headers).await.into_response();
    }

    let allowed_methods = [Method::GET, Method::HEAD, Method::POST];
    if !allowed_methods.contains(&method) {
        warn!("Method not allowed: {}", method);
        return (StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed").into_response();
    }

    match http_request(req_url, method, headers, body).await {
        Ok(resp) => {
            debug!("Request successful, streaming response");
            stream_response(resp).await.into_response()
        }
        Err(e) => {
            error!("Request failed: {:#}", e);
            let mut msg = String::new();
            let mut slot: Option<&dyn std::error::Error> = Some(&e);
            while let Some(err) = slot {
                msg.push_str(&err.to_string());
                msg.push('\n');
                slot = err.source();
            }
            debug!("Request error chain:\n{}", msg.trim());
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send request").into_response()
        }
    }
}

// 入口函数 - 处理所有请求
async fn entry(
    uri: Uri,
    method: Method,
    headers: HeaderMap,
    query: RawQuery,
    body: Body,
) -> Response<Body> {
    let path = uri.path();
    debug!("Incoming request: {} {}", method, path);

    // POST 请求体缓冲后转发（git 智能协议 negotiate/pack 数据等；缓冲以便重定向时重放，
    // git 请求体通常 KB 级可接受），转 reqwest::Body 不做额外整体缓冲
    let req_body: Option<axum::body::Bytes> = if method == Method::POST {
        match axum::body::to_bytes(body, 64 * 1024 * 1024).await {
            Ok(buf) => Some(buf),
            Err(e) => {
                warn!("Failed to read request body: {}", e);
                None
            }
        }
    } else {
        None
    };

    // 处理根路径
    if path == "/" {
        return handler_index().await.into_response();
    }

    // 处理 favicon.ico
    if path == "/favicon.ico" {
        debug!("Serving favicon");
        return handler_favicon().await.into_response();
    }

    // 处理 robots.txt
    if path == "/robots.txt" {
        debug!("Serving robots.txt");
        return handler_robots().await.into_response();
    }

    // 取出网址
    let mut redirect_url = path.trim_start_matches('/').to_string();

    // 解码 URL（仅解码路径部分，单层解码；失败时使用原始值）
    let original_url = redirect_url.clone();
    redirect_url = urlencoding::decode(&redirect_url)
        .unwrap_or_default()
        .to_string();
    if original_url != redirect_url {
        debug!("URL decoded: {} -> {}", original_url, redirect_url);
    }

    // 添加查询参数（原样透传，不解码）
    if let Some(query_str) = query.0 {
        if !query_str.is_empty() {
            redirect_url.push('?');
            redirect_url.push_str(&query_str);
            debug!("Added query parameters: {}", query_str);
        }
    }

    // 处理特殊的清理逻辑：如果路径以 https:/https:// 开头（某些客户端转义结果）
    if redirect_url.starts_with("http:/http://") {
        redirect_url = redirect_url.replacen("http:/http://", "http://", 1);
    } else if redirect_url.starts_with("https:/https://") {
        redirect_url = redirect_url.replacen("https:/https://", "https://", 1);
    } else if redirect_url.starts_with("http:/") && !redirect_url.starts_with("http://") {
        redirect_url = redirect_url.replacen("http:/", "http://", 1);
    } else if redirect_url.starts_with("https:/") && !redirect_url.starts_with("https://") {
        redirect_url = redirect_url.replacen("https:/", "https://", 1);
    }

    // 检查是否已经是完整的 URL
    if redirect_url.starts_with("http://") || redirect_url.starts_with("https://") {
        debug!("Processing complete URL: {}", redirect_url);
        return do_request(
            &redirect_url,
            method,
            headers,
            req_body.map(reqwest::Body::from),
        )
        .await
        .into_response();
    }

    // 处理有 Referer 的情况
    if let Some(referer) = headers.get(header::REFERER) {
        if let Ok(referer_str) = referer.to_str() {
            if let Ok(referer_url) = Url::parse(referer_str) {
                let origin_url = format!(
                    "{}://{}",
                    referer_url.scheme(),
                    referer_url.host_str().unwrap_or("")
                );
                let full_url = format!("{}/{}", origin_url, redirect_url);
                debug!(
                    "Processing URL with referer: {} (from {})",
                    full_url, referer_str
                );
                return do_request(
                    &full_url,
                    method,
                    headers,
                    req_body.map(reqwest::Body::from),
                )
                .await
                .into_response();
            }
        }
    }

    // 去除多余的斜杠并补充协议
    redirect_url = redirect_url.trim_start_matches('/').to_string();
    if let Some(first_part) = redirect_url.split('/').next() {
        if is_domain(first_part) {
            let full_url = format!("https://{}", redirect_url);
            debug!("Processing domain URL: {}", full_url);
            return do_request(
                &full_url,
                method,
                headers,
                req_body.map(reqwest::Body::from),
            )
            .await
            .into_response();
        }
    }

    warn!("Invalid URL requested: {}", redirect_url);
    (StatusCode::BAD_REQUEST, "Invalid URL").into_response()
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // 设置全局配置
    CONFIG.set(args).unwrap();
    let config = get_config();

    // 初始化日志
    let log_level = if config.verbose {
        "filetas=debug,tower_http=debug"
    } else if config.quiet {
        "filetas=warn,tower_http=warn"
    } else {
        "filetas=info,tower_http=info"
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_level.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting filetas server v{}", env!("CARGO_PKG_VERSION"));
    info!("Configuration:");
    info!("  Host: {}", config.host);
    info!("  Port: {}", config.port);
    info!("  Title: {}", config.title);
    info!("  Template Dir: {}", config.template_dir);
    info!("  User Agent: {}", config.user_agent);

    // 验证模板目录（不存在时使用内置页面）
    let template_path = format!("{}/index.html", config.template_dir);
    if !std::path::Path::new(&template_path).exists() {
        warn!(
            "Template file not found: {}, using built-in fallback page",
            template_path
        );
    }

    let app = Router::new().fallback(entry).layer(
        ServiceBuilder::new().layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        ),
    );

    let addr = SocketAddr::new(
        config.host.parse().unwrap_or_else(|e| {
            error!("Invalid host address '{}': {}", config.host, e);
            std::process::exit(1);
        }),
        config.port,
    );

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            error!("Failed to bind to {}:{}: {}", config.host, config.port, e);
            std::process::exit(1);
        });

    info!("Server listening on {}", listener.local_addr().unwrap());
    info!(
        "Access the web interface at: http://{}:{}",
        if config.host == "0.0.0.0" {
            "localhost"
        } else {
            &config.host
        },
        config.port
    );

    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }
}
