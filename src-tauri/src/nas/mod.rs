// NAS 服务模块
// 提供 Web UI、WebDAV、FTP 三种协议访问

use crate::drives::DriveManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NasInfo {
    pub running: bool,
    pub web_ui_url: String,
    pub webdav_url: String,
    pub ftp_url: String,
    pub local_ip: String,
    pub port: u16,
}

pub struct NasServer {
    port: u16,
    drive_manager: Arc<RwLock<DriveManager>>,
    info: NasInfo,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl NasServer {
    pub fn new(port: u16, drive_manager: Arc<RwLock<DriveManager>>) -> Self {
        let local_ip = get_local_ip();
        Self {
            port,
            drive_manager,
            info: NasInfo {
                running: false,
                web_ui_url: String::new(),
                webdav_url: String::new(),
                ftp_url: String::new(),
                local_ip: local_ip.clone(),
                port,
            },
            shutdown_tx: None,
        }
    }

    pub async fn start(&mut self) -> Result<NasInfo, String> {
        let ip = self.info.local_ip.clone();
        let port = self.port;

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        let dm = self.drive_manager.clone();

        // 启动 Web 服务器（Web UI + WebDAV 共用端口）
        let web_port = port;
        let dm_clone = dm.clone();

        tokio::spawn(async move {
            use actix_web::{web, App, HttpServer, HttpResponse, middleware};
            use actix_cors::Cors;

            let dm_for_app = dm_clone.clone();

            let server = HttpServer::new(move || {
                let cors = Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header();

                App::new()
                    .wrap(cors)
                    .app_data(web::Data::new(dm_for_app.clone()))
                    // Web UI 静态文件
                    .route("/", web::get().to(serve_index))
                    .route("/css/{filename}", web::get().to(serve_css))
                    .route("/js/{filename}", web::get().to(serve_js))
                    // WebDAV 接口
                    .route("/dav/{path:.*}", web::get().to(webdav_get))
                    .route("/dav/{path:.*}", web::put().to(webdav_put))
                    .route("/dav/{path:.*}", web::method(actix_web::http::Method::PROPFIND).to(webdav_propfind))
                    .route("/dav/{path:.*}", web::delete().to(webdav_delete))
                    .route("/dav/{path:.*}", web::method(actix_web::http::Method::MKCOL).to(webdav_mkcol))
                    // API 接口（给 Web UI 用）
                    .route("/api/files/{drive_id}", web::get().to(api_list_files))
                    .route("/api/download/{drive_id}/{file_id}", web::get().to(api_download))
                    .route("/api/search", web::get().to(api_search))
                    .route("/api/quota/{drive_id}", web::get().to(api_quota))
                    .route("/api/drives", web::get().to(api_drives))
            })
            .bind(format!("0.0.0.0:{}", web_port))
            .expect("Failed to bind NAS port")
            .shutdown_timeout(5);

            // 监听关闭信号
            tokio::select! {
                _ = server.run() => {}
                _ = async {
                    let _ = shutdown_rx.await;
                } => {}
            }
        });

        // mDNS 广播（局域网发现）
        let mdns_ip = ip.clone();
        tokio::spawn(async move {
            start_mdns_advertisement(&mdns_ip, port).await;
        });

        self.info = NasInfo {
            running: true,
            web_ui_url: format!("http://{}:{}", ip, port),
            webdav_url: format!("http://{}:{}/dav", ip, port),
            ftp_url: format!("ftp://{}:{}", ip, port + 1),
            local_ip: ip,
            port,
        };

        Ok(self.info.clone())
    }

    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.info.running = false;
    }

    pub fn info(&self) -> NasInfo {
        self.info.clone()
    }
}

// ======================== HTTP 处理函数 ========================

async fn serve_index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("../../web/index.html"))
}

async fn serve_css(path: web::Path<String>) -> HttpResponse {
    let filename = path.into_inner();
    let content = match filename.as_str() {
        "base.css" => include_str!("../../web/css/base.css"),
        "layout.css" => include_str!("../../web/css/layout.css"),
        "components.css" => include_str!("../../web/css/components.css"),
        "pages.css" => include_str!("../../web/css/pages.css"),
        _ => return HttpResponse::NotFound().finish(),
    };
    HttpResponse::Ok()
        .content_type("text/css; charset=utf-8")
        .body(content)
}

async fn serve_js(path: web::Path<String>) -> HttpResponse {
    let filename = path.into_inner();
    let content = match filename.as_str() {
        "utils.js" => include_str!("../../web/js/utils.js"),
        "storage.js" => include_str!("../../web/js/storage.js"),
        "api.js" => include_str!("../../web/js/api.js"),
        "router.js" => include_str!("../../web/js/router.js"),
        "app.js" => include_str!("../../web/js/app.js"),
        _ => return HttpResponse::NotFound().finish(),
    };
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .body(content)
}

// ======================== WebDAV 处理 ========================

async fn webdav_propfind(
    _req: actix_web::HttpRequest,
    _body: String,
) -> HttpResponse {
    // TODO: 实现 WebDAV PROPFIND
    HttpResponse::MultiStatus()
        .content_type("application/xml; charset=utf-8")
        .body("<?xml version=\"1.0\" encoding=\"utf-8\"?><d:multistatus xmlns:d=\"DAV:\"></d:multistatus>")
}

async fn webdav_get(
    _req: actix_web::HttpRequest,
) -> HttpResponse {
    // TODO: 实现 WebDAV GET（下载文件）
    HttpResponse::NotFound().finish()
}

async fn webdav_put(
    _req: actix_web::HttpRequest,
    _body: actix_web::web::Bytes,
) -> HttpResponse {
    // TODO: 实现 WebDAV PUT（上传文件）
    HttpResponse::Created().finish()
}

async fn webdav_delete(
    _req: actix_web::HttpRequest,
) -> HttpResponse {
    // TODO: 实现 WebDAV DELETE
    HttpResponse::NoContent().finish()
}

async fn webdav_mkcol(
    _req: actix_web::HttpRequest,
) -> HttpResponse {
    // TODO: 实现 WebDAV MKCOL（创建文件夹）
    HttpResponse::Created().finish()
}

// ======================== API 处理函数（给 Web UI 用）========================

async fn api_list_files(
    req: actix_web::HttpRequest,
    dm: web::Data<Arc<RwLock<DriveManager>>>,
) -> HttpResponse {
    let drive_id = req.match_info().get("drive_id").unwrap_or("");
    let path = req.query_string()
        .split('&')
        .find(|s| s.starts_with("path="))
        .and_then(|s| s.strip_prefix("path="))
        .unwrap_or("/");

    let dm_read = dm.read().await;
    let drive = match dm_read.get_drive(drive_id) {
        Some(d) => d,
        None => return HttpResponse::NotFound().json(serde_json::json!({"error": "网盘未找到"})),
    };

    let d = drive.read().await;
    match d.list_files(path).await {
        Ok(files) => HttpResponse::Ok().json(serde_json::json!({"list": files})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e})),
    }
}

async fn api_download(
    req: actix_web::HttpRequest,
    dm: web::Data<Arc<RwLock<DriveManager>>>,
) -> HttpResponse {
    let drive_id = req.match_info().get("drive_id").unwrap_or("");
    let file_id = req.match_info().get("file_id").unwrap_or("");

    let dm_read = dm.read().await;
    let drive = match dm_read.get_drive(drive_id) {
        Some(d) => d,
        None => return HttpResponse::NotFound().finish(),
    };

    let d = drive.read().await;
    match d.get_download_link(file_id).await {
        Ok(url) => HttpResponse::Found()
            .append_header(("Location", url))
            .finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

async fn api_search(
    req: actix_web::HttpRequest,
    dm: web::Data<Arc<RwLock<DriveManager>>>,
) -> HttpResponse {
    let keyword = req.query_string()
        .split('&')
        .find(|s| s.starts_with("key="))
        .and_then(|s| s.strip_prefix("key="))
        .unwrap_or("");

    if keyword.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "请输入搜索词"}));
    }

    let dm_read = dm.read().await;
    let drives = dm_read.get_all_drives();
    let mut results = Vec::new();

    for (_id, drive) in &drives {
        let d = drive.read().await;
        if d.is_logged_in() {
            if let Ok(files) = d.search_files(keyword).await {
                results.extend(files);
            }
        }
    }

    HttpResponse::Ok().json(serde_json::json!({"list": results}))
}

async fn api_quota(
    req: actix_web::HttpRequest,
    dm: web::Data<Arc<RwLock<DriveManager>>>,
) -> HttpResponse {
    let drive_id = req.match_info().get("drive_id").unwrap_or("");
    let dm_read = dm.read().await;
    let drive = match dm_read.get_drive(drive_id) {
        Some(d) => d,
        None => return HttpResponse::NotFound().finish(),
    };
    let d = drive.read().await;
    match d.get_quota().await {
        Ok(q) => HttpResponse::Ok().json(q),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e})),
    }
}

async fn api_drives(
    dm: web::Data<Arc<RwLock<DriveManager>>>,
) -> HttpResponse {
    let dm_read = dm.read().await;
    let drives = dm_read.get_connected_drives().await;
    let list: Vec<serde_json::Value> = drives.iter().map(|(id, dt, logged)| {
        serde_json::json!({"id": id, "type": dt, "logged_in": logged})
    }).collect();
    HttpResponse::Ok().json(list)
}

// ======================== 工具函数 ========================

fn get_local_ip() -> String {
    // 获取本机局域网 IP
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok();
    if let Some(s) = socket {
        if s.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = s.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

async fn start_mdns_advertisement(ip: &str, port: u16) {
    // mDNS 服务注册，让局域网内其他设备自动发现
    use mdns_sd::{ServiceDaemon, ServiceInfo};

    let mdns = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(_) => return,
    };

    let service_type = "_litedisk._tcp.local.";
    let instance_name = format!("轻盘-{}", &ip.replace('.', "-"));
    let hostname = format!("{}.local.", ip.replace('.', "-"));

    let service = match ServiceInfo::new(
        service_type,
        &instance_name,
        &hostname,
        ip,
        port,
        &[("version=1.0", "app=litedisk")][..])
    ) {
        Ok(s) => s,
        Err(_) => return,
    };

    let _ = mdns.register(service);

    // 保持 mDNS 服务运行
    tokio::time::sleep(tokio::time::Duration::from_secs(86400 * 365)).await;
}
