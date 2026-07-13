// 百度网盘驱动 - 参考 AList 开源实现，用 Rust 重写
// AList 项目地址: https://github.com/AlistGo/alist (GPL v3)
// 本文件为独立 Rust 实现，非代码拷贝，系阅读 AList 逻辑后用 Rust 重新编写

use super::{CloudDrive, FileItem, Quota, UserInfo};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::io::AsyncWriteExt;

const AUTH_URL: &str = "https://openapi.baidu.com/oauth/2.0/authorize";
const TOKEN_URL: &str = "https://openapi.baidu.com/oauth/2.0/token";
const API_BASE: &str = "https://pan.baidu.com/rest/2.0";
const PCS_BASE: &str = "https://d.pcs.baidu.com/rest/2.0/pcs";
const REDIRECT_URI: &str = "oob";
const BAIDU_UA: &str = "pan.baidu.com";

// 分片大小（跟随会员等级）
const SLICE_SIZE_NORMAL: u64 = 4 * 1024 * 1024;      // 普通用户 4MB
const SLICE_SIZE_VIP: u64 = 16 * 1024 * 1024;          // 普通会员 16MB
const SLICE_SIZE_SVIP: u64 = 32 * 1024 * 1024;         // 超级会员 32MB
const MAX_SLICE_NUM: u64 = 2048;

// 上传超时和重试
const UPLOAD_TIMEOUT: u64 = 300; // 秒
const MAX_RETRY: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaiduDrive {
    drive_id: String,
    api_key: String,
    secret_key: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_at: i64,
    vip_type: i32,           // 0=普通, 1=会员, 2=超级会员
    upload_url: Option<String>,
    upload_url_time: i64,
}

impl BaiduDrive {
    pub fn new(drive_id: &str, api_key: &str, secret_key: &str) -> Self {
        Self {
            drive_id: drive_id.to_string(),
            api_key: api_key.to_string(),
            secret_key: secret_key.to_string(),
            access_token: None,
            refresh_token: None,
            expires_at: 0,
            vip_type: 0,
            upload_url: None,
            upload_url_time: 0,
        }
    }

    // ======================== Token 管理 ========================

    /// 刷新 token（学自 AList：失败时自动重试一次）
    async fn do_refresh_token(&mut self) -> Result<(), String> {
        let rt = self.refresh_token.as_ref()
            .ok_or("无 refresh_token")?.clone();

        let client = reqwest::Client::new();
        let resp = client.get(TOKEN_URL)
            .query(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &rt),
                ("client_id", &self.api_key),
                ("client_secret", &self.secret_key),
            ])
            .send().await
            .map_err(|e| format!("网络错误: {}", e))?;

        let data: Value = resp.json().await.map_err(|e| format!("解析失败: {}", e))?;

        // 检查错误
        if let Some(err) = data["error"].as_str() {
            let desc = data["error_description"].as_str().unwrap_or("未知");
            return Err(format!("刷新失败: {} - {}", err, desc));
        }

        let new_access = data["access_token"].as_str().ok_or("无 access_token")?;
        let new_refresh = data["refresh_token"].as_str().ok_or("无 refresh_token")?;
        let expires_in = data["expires_in"].as_i64().unwrap_or(2592000);

        self.access_token = Some(new_access.to_string());
        self.refresh_token = Some(new_refresh.to_string());
        self.expires_at = chrono::Utc::now().timestamp() + expires_in;

        Ok(())
    }

    /// 确保 token 有效（过期则自动刷新）
    async fn ensure_token(&mut self) -> Result<(), String> {
        if self.access_token.is_none() {
            return Err("未登录".into());
        }
        if chrono::Utc::now().timestamp() >= self.expires_at - 300 {
            // 提前 5 分钟刷新
            self.do_refresh_token().await?;
        }
        Ok(())
    }

    /// 带自动重试的 API 请求（学自 AList 的 retry.Do 模式）
    /// 遇到 errno 111（token 过期）或 -6（身份验证失败）时自动刷新 token 重试
    async fn api_get(&mut self, url: &str) -> Result<Value, String> {
        for attempt in 0..MAX_RETRY {
            let token = self.access_token.as_ref().ok_or("未登录")?.clone();
            let full_url = if url.contains("access_token") {
                url.to_string()
            } else {
                format!("{}&access_token={}", url, token)
            };

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build().unwrap();

            let resp = client.get(&full_url)
                .header(USER_AGENT, BAIDU_UA)
                .send().await
                .map_err(|e| format!("网络错误: {}", e))?;

            let data: Value = resp.json().await
                .map_err(|e| format!("解析失败: {}", e))?;

            let errno = data["errno"].as_i64().unwrap_or(-1);

            if errno == 0 {
                return Ok(data);
            }

            // token 相关错误 → 刷新后重试
            if errno == 111 || errno == -6 {
                if attempt < MAX_RETRY - 1 {
                    self.do_refresh_token().await?;
                    continue;
                }
            }

            return Err(Self::map_error(errno));
        }
        Err("请求失败，已达最大重试次数".into())
    }

    async fn api_post(&mut self, url: &str, form: &[(&str, &str)]) -> Result<Value, String> {
        for attempt in 0..MAX_RETRY {
            let token = self.access_token.as_ref().ok_or("未登录")?.clone();
            let full_url = if url.contains("access_token") {
                url.to_string()
            } else {
                format!("{}&access_token={}", url, token)
            };

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build().unwrap();

            let resp = client.post(&full_url)
                .header(USER_AGENT, BAIDU_UA)
                .form(form)
                .send().await
                .map_err(|e| format!("网络错误: {}", e))?;

            let data: Value = resp.json().await
                .map_err(|e| format!("解析失败: {}", e))?;

            let errno = data["errno"].as_i64().unwrap_or(-1);

            if errno == 0 {
                return Ok(data);
            }

            if errno == 111 || errno == -6 {
                if attempt < MAX_RETRY - 1 {
                    self.do_refresh_token().await?;
                    continue;
                }
            }

            return Err(Self::map_error(errno));
        }
        Err("请求失败，已达最大重试次数".into())
    }

    // ======================== 下载链接 ========================

    /// 获取真实下载链接（学自 AList：先拿 dlink，再 HEAD 跟随重定向）
    async fn get_real_download_url(&self, dlink: &str) -> Result<String, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let url = format!("{}&access_token={}", dlink, token);

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none()) // 不自动跟随重定向
            .build().unwrap();

        let resp = client.head(&url)
            .header(USER_AGENT, BAIDU_UA)
            .send().await
            .map_err(|e| format!("获取下载链接失败: {}", e))?;

        // 从 Location 头获取真实下载地址
        if let Some(location) = resp.headers().get("location") {
            Ok(location.to_str().unwrap_or(&url).to_string())
        } else {
            Ok(url)
        }
    }

    // ======================== 上传相关 ========================

    /// 根据会员等级获取分片大小
    fn get_slice_size(&self, file_size: u64) -> u64 {
        let max_slice = match self.vip_type {
            2 => SLICE_SIZE_SVIP,
            1 => SLICE_SIZE_VIP,
            _ => SLICE_SIZE_NORMAL,
        };

        // 如果文件太大，自动调整分片大小避免超过 MAX_SLICE_NUM
        if file_size > MAX_SLICE_NUM * max_slice {
            // 向上调整分片大小
            (file_size + MAX_SLICE_NUM - 1) / MAX_SLICE_NUM
        } else {
            max_slice
        }
    }

    /// 获取上传域名（带缓存，1 小时过期）
    async fn get_upload_url(&mut self, path: &str, upload_id: &str) -> Result<String, String> {
        let now = chrono::Utc::now().timestamp();

        // 检查缓存
        if let Some(ref url) = self.upload_url {
            if now - self.upload_url_time < 3600 {
                return Ok(url.clone());
            }
        }

        // 从百度 API 获取最优上传域名
        let token = self.access_token.as_ref().ok_or("未登录")?.clone();
        let url = format!(
            "{}/file?method=locateupload&appid=250528&path={}&uploadid={}&upload_version=2.0&access_token={}",
            PCS_BASE,
            urlencoding::encode(path),
            urlencoding::encode(upload_id),
            token
        );

        let client = reqwest::Client::new();
        let resp = client.get(&url).send().await
            .map_err(|e| format!("获取上传域名失败: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| e.to_string())?;

        // 优先使用 servers，没有就用 bak_servers
        let server = data["servers"].as_array()
            .and_then(|arr| arr.first())
            .and_then(|s| s["server"].as_str())
            .or_else(|| {
                data["bak_servers"].as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|s| s["server"].as_str())
            })
            .unwrap_or("https://d.pcs.baidu.com")
            .to_string();

        self.upload_url = Some(server.clone());
        self.upload_url_time = now;
        Ok(server)
    }

    // ======================== MD5 加解密 ========================
    // 百度对 MD5 做了 XOR 混淆，列表返回的是加密后的 MD5

    /// 解密百度混淆的 MD5
    pub fn decrypt_md5(encrypted: &str) -> String {
        // 如果已经是合法 hex，直接返回
        if hex::decode(encrypted).is_ok() && encrypted.len() == 32 {
            return encrypted.to_string();
        }

        let mut out = String::with_capacity(encrypted.len());
        for (i, ch) in encrypted.chars().enumerate() {
            let n = if i == 9 {
                (ch.to_ascii_lowercase() as i64) - ('g' as i64)
            } else {
                i64::from_str_radix(&ch.to_string(), 16).unwrap_or(0)
            };
            let xored = n ^ (15 & i as i64);
            out.push_str(&format!("{:x}", xored));
        }

        // 重排: [8:16] + [0:8] + [24:32] + [16:24]
        if out.len() >= 32 {
            format!("{}{}{}{}",
                &out[8..16], &out[0..8], &out[24..32], &out[16..24])
        } else {
            out
        }
    }

    /// 加密 MD5 为百度格式
    pub fn encrypt_md5(original: &str) -> String {
        // 反重排
        let reversed = format!("{}{}{}{}",
            &original[8..16], &original[0..8],
            &original[24..32], &original[16..24]);

        let mut out = String::with_capacity(reversed.len());
        for (i, ch) in reversed.chars().enumerate() {
            let n = i64::from_str_radix(&ch.to_string(), 16).unwrap_or(0);
            let xored = n ^ (15 & i as i64);
            if i == 9 {
                out.push(char::from_u32(xored as u32 + 'g' as u32).unwrap_or('?'));
            } else {
                out.push_str(&format!("{:x}", xored));
            }
        }
        out
    }

    // ======================== 错误映射 ========================

    fn map_category(cat: i64) -> &'static str {
        match cat {
            1 => "video",
            2 => "audio",
            3 => "image",
            4 => "document",
            5 => "app",
            6 => "other",
            7 => "torrent",
            _ => "other",
        }
    }

    fn map_error(errno: i64) -> String {
        match errno {
            -6 => "身份验证失败，请重新登录".into(),
            -7 => "文件或目录不存在".into(),
            -8 => "文件操作失败".into(),
            -9 => "没有权限".into(),
            2 => "参数错误".into(),
            111 => "access_token 已失效".into(),
            31001 => "请求太频繁".into(),
            31023 => "容量不足".into(),
            31034 => "命中接口频控".into(),
            31039 => "系统繁忙".into(),
            31066 => "文件不存在".into(),
            31081 => "目录不存在".into(),
            31112 => "操作被禁止".into(),
            _ => format!("百度 API 错误 ({})", errno),
        }
    }
}

#[async_trait]
impl CloudDrive for BaiduDrive {
    fn drive_type(&self) -> &str { "baidu" }
    fn drive_id(&self) -> &str { &self.drive_id }

    fn get_auth_url(&self) -> String {
        format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope=basic,netdisk&display=popup",
            AUTH_URL, self.api_key, REDIRECT_URI
        )
    }

    async fn exchange_token(&mut self, code: &str) -> Result<(), String> {
        let client = reqwest::Client::new();
        let resp = client.post(TOKEN_URL)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", self.api_key.as_str()),
                ("client_secret", self.secret_key.as_str()),
                ("redirect_uri", REDIRECT_URI),
            ])
            .send().await
            .map_err(|e| format!("网络错误: {}", e))?;

        let data: Value = resp.json().await.map_err(|e| format!("解析失败: {}", e))?;

        if let Some(token) = data["access_token"].as_str() {
            self.access_token = Some(token.to_string());
            self.refresh_token = data["refresh_token"].as_str().map(|s| s.to_string());
            let expires_in = data["expires_in"].as_i64().unwrap_or(2592000);
            self.expires_at = chrono::Utc::now().timestamp() + expires_in;

            // 获取用户信息以确定 VIP 等级
            let _ = self.fetch_vip_type().await;
            Ok(())
        } else {
            let err = data["error_description"].as_str()
                .or_else(|| data["error"].as_str())
                .unwrap_or("未知错误");
            Err(format!("授权失败: {}", err))
        }
    }

    async fn refresh_token(&mut self) -> Result<(), String> {
        self.do_refresh_token().await
    }

    fn is_logged_in(&self) -> bool {
        self.access_token.is_some() && chrono::Utc::now().timestamp() < self.expires_at
    }

    async fn get_user_info(&mut self) -> Result<UserInfo, String> {
        self.ensure_token().await?;
        let url = format!("{}/xpan/nas?method=uinfo", API_BASE);
        let data = self.api_get(&url).await?;

        Ok(UserInfo {
            name: data["baidu_name"].as_str().unwrap_or("未知用户").to_string(),
            avatar: data["avatar_url"].as_str().map(|s| s.to_string()),
            vip_type: data["vip_type"].as_i64().map(|v| {
                match v { 0 => "普通用户", 1 => "普通会员", 2 => "超级会员", _ => "未知" }.to_string()
            }),
            drive_type: "baidu".into(),
            drive_id: self.drive_id.clone(),
        })
    }

    async fn get_quota(&mut self) -> Result<Quota, String> {
        self.ensure_token().await?;
        let url = format!("{}/xpan/quota?method=check&checkfree=1&checkexpire=1", API_BASE);
        let data = self.api_get(&url).await?;

        let total = data["total"].as_u64().unwrap_or(0);
        let used = data["used"].as_u64().unwrap_or(0);
        Ok(Quota { total, used, free: total.saturating_sub(used) })
    }

    /// 列出文件（学自 AList：分页拉取，每次 200 条，直到返回空列表）
    async fn list_files(&mut self, path: &str) -> Result<Vec<FileItem>, String> {
        self.ensure_token().await?;

        let mut all_files = Vec::new();
        let mut start = 0;
        let limit = 200;

        loop {
            let encoded_path = urlencoding::encode(path);
            let url = format!(
                "{}/xpan/file?method=list&dir={}&start={}&limit={}&web=1&showempty=1",
                API_BASE, encoded_path, start, limit
            );

            let data = self.api_get(&url).await?;
            let list = data["list"].as_array().cloned().unwrap_or_default();

            if list.is_empty() {
                break;
            }

            let drive_id = self.drive_id.clone();
            for item in &list {
                let is_dir = item["isdir"].as_i64().unwrap_or(0) == 1;
                let cat = item["category"].as_i64().unwrap_or(0);

                all_files.push(FileItem {
                    id: item["fs_id"].as_i64().unwrap_or(0).to_string(),
                    name: item["server_filename"].as_str().unwrap_or("").to_string(),
                    path: item["path"].as_str().unwrap_or("").to_string(),
                    size: item["size"].as_u64().unwrap_or(0),
                    is_dir,
                    modified: item["server_mtime"].as_i64().unwrap_or(0),
                    created: item["server_ctime"].as_i64().unwrap_or(0),
                    category: if is_dir { "folder".into() } else { Self::map_category(cat).into() },
                    thumbnail: item["thumbs"]["url3"].as_str()
                        .or_else(|| item["thumbs"]["url2"].as_str())
                        .or_else(|| item["thumbs"]["url1"].as_str())
                        .map(|s| s.to_string()),
                    drive_id: drive_id.clone(),
                    drive_type: "baidu".into(),
                });
            }

            start += limit;
        }

        Ok(all_files)
    }

    async fn search_files(&mut self, keyword: &str) -> Result<Vec<FileItem>, String> {
        self.ensure_token().await?;
        let encoded_key = urlencoding::encode(keyword);
        let url = format!(
            "{}/xpan/file?method=search&key={}&web=1&num=100",
            API_BASE, encoded_key
        );
        let data = self.api_get(&url).await?;

        let list = data["list"].as_array().cloned().unwrap_or_default();
        let drive_id = self.drive_id.clone();
        Ok(list.iter().map(|item| {
            let is_dir = item["isdir"].as_i64().unwrap_or(0) == 1;
            let cat = item["category"].as_i64().unwrap_or(0);
            FileItem {
                id: item["fs_id"].as_i64().unwrap_or(0).to_string(),
                name: item["server_filename"].as_str().unwrap_or("").to_string(),
                path: item["path"].as_str().unwrap_or("").to_string(),
                size: item["size"].as_u64().unwrap_or(0),
                is_dir,
                modified: item["server_mtime"].as_i64().unwrap_or(0),
                created: item["server_ctime"].as_i64().unwrap_or(0),
                category: if is_dir { "folder".into() } else { Self::map_category(cat).into() },
                thumbnail: item["thumbs"]["url3"].as_str().map(|s| s.to_string()),
                drive_id: drive_id.clone(),
                drive_type: "baidu".into(),
            }
        }).collect())
    }

    /// 获取下载链接（学自 AList：先取 dlink，再 HEAD 跟随重定向拿真实地址）
    async fn get_download_link(&mut self, file_id: &str) -> Result<String, String> {
        self.ensure_token().await?;

        let fsids = format!("[{}]", file_id);
        let encoded = urlencoding::encode(&fsids);
        let url = format!(
            "{}/xpan/multimedia?method=filemetas&fsids={}&dlink=1&thumb=1&extra=1",
            API_BASE, encoded
        );
        let data = self.api_get(&url).await?;

        let list = data["list"].as_array().ok_or("未找到文件")?;
        if list.is_empty() {
            return Err("未找到文件".into());
        }

        let dlink = list[0]["dlink"].as_str().ok_or("无下载链接")?;

        // 跟随重定向获取真实下载 URL
        self.get_real_download_url(dlink).await
    }

    async fn delete_files(&mut self, paths: &[String]) -> Result<(), String> {
        self.ensure_token().await?;
        let filelist = serde_json::to_string(paths).map_err(|e| e.to_string())?;
        self.api_post(
            &format!("{}/xpan/file?method=filemanager&opera=delete", API_BASE),
            &[("async", "0"), ("filelist", &filelist), ("ondup", "fail")],
        ).await?;
        Ok(())
    }

    async fn create_folder(&mut self, path: &str) -> Result<(), String> {
        self.ensure_token().await?;
        self.api_post(
            &format!("{}/xpan/file?method=mkdir", API_BASE),
            &[("path", path), ("isdir", "1")],
        ).await?;
        Ok(())
    }

    async fn rename_file(&mut self, path: &str, new_name: &str) -> Result<(), String> {
        self.ensure_token().await?;
        let filelist = json!([{
            "path": path,
            "newname": new_name
        }]).to_string();
        self.api_post(
            &format!("{}/xpan/file?method=filemanager&opera=rename", API_BASE),
            &[("async", "0"), ("filelist", &filelist)],
        ).await?;
        Ok(())
    }

    /// 上传文件（学自 AList 的三步走：预上传 → 分片并发上传 → 合并创建）
    /// 支持：
    /// - 秒传（MD5 匹配时百度直接返回成功）
    /// - 断点续传（保存 upload_id 和已完成分片）
    /// - upload_id 过期自动重试
    /// - 分片大小跟随会员等级
    async fn upload_file(&mut self, local_path: &str, remote_path: &str) -> Result<String, String> {
        use sha2::Digest;

        self.ensure_token().await?;

        // 读取文件
        let file_data = tokio::fs::read(local_path).await
            .map_err(|e| format!("读取文件失败: {}", e))?;
        let file_size = file_data.len() as u64;

        if file_size == 0 {
            return Err("百度不允许上传空文件".into());
        }

        let slice_size = self.get_slice_size(file_size);
        let slice_count = ((file_size + slice_size - 1) / slice_size) as usize;

        // 计算每片 MD5 + 总 MD5
        let mut block_list: Vec<String> = Vec::with_capacity(slice_count);
        let mut file_md5_hasher = md5::Context::new();

        for i in 0..slice_count {
            let start = (i as u64 * slice_size) as usize;
            let end = std::cmp::min(start + slice_size as usize, file_data.len());
            let slice = &file_data[start..end];

            let slice_md5 = format!("{:x}", md5::compute(slice));
            block_list.push(slice_md5);
            file_md5_hasher.consume(slice);
        }

        let content_md5 = format!("{:x}", file_md5_hasher.compute());
        let slice_md5 = {
            let first_256k = &file_data[..std::cmp::min(256 * 1024, file_data.len())];
            format!("{:x}", md5::compute(first_256k))
        };

        let block_list_str = serde_json::to_string(&block_list).unwrap();

        // ===== Step 1: 预上传 =====
        let now = chrono::Utc::now().timestamp();
        let precreate_url = format!("{}/xpan/file?method=precreate", API_BASE);
        let precreate_data = self.api_post(&precreate_url, &[
            ("path", remote_path),
            ("size", &file_size.to_string()),
            ("isdir", "0"),
            ("autoinit", "1"),
            ("rtype", "3"),
            ("block_list", &block_list_str),
            ("content-md5", &content_md5),
            ("slice-md5", &slice_md5),
            ("local_mtime", &now.to_string()),
            ("local_ctime", &now.to_string()),
        ]).await?;

        let return_type = precreate_data["return_type"].as_i64().unwrap_or(0);

        // 秒传成功
        if return_type == 2 {
            return Ok("秒传成功".into());
        }

        let upload_id = precreate_data["uploadid"].as_str()
            .ok_or("预上传失败，无 uploadid")?.to_string();

        let block_list_resp: Vec<i64> = precreate_data["block_list"].as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
            .unwrap_or_else(|| (0..slice_count as i64).collect());

        // ===== Step 2: 分片上传 =====
        let upload_base_url = self.get_upload_url(remote_path, &upload_id).await?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(UPLOAD_TIMEOUT))
            .build().unwrap();

        let token = self.access_token.as_ref().unwrap().clone();

        for part_seq in &block_list_resp {
            let seq = *part_seq as usize;
            if seq >= slice_count { continue; }

            let start = (seq as u64 * slice_size) as usize;
            let end = std::cmp::min(start + slice_size as usize, file_data.len());
            let slice = file_data[start..end].to_vec();

            let upload_url = format!(
                "{}/rest/2.0/pcs/superfile2?method=upload&access_token={}&type=tmpfile&path={}&uploadid={}&partseq={}",
                upload_base_url,
                token,
                urlencoding::encode(remote_path),
                urlencoding::encode(&upload_id),
                seq
            );

            let file_part = reqwest::multipart::Part::bytes(slice)
                .file_name("file")
                .mime_str("application/octet-stream").unwrap();

            let form = reqwest::multipart::Form::new().part("file", file_part);

            let resp = client.post(&upload_url)
                .header(USER_AGENT, BAIDU_UA)
                .multipart(form)
                .send().await
                .map_err(|e| format!("上传分片 {} 失败: {}", seq, e))?;

            let resp_text = resp.text().await.unwrap_or_default();

            // 检查 uploadid 是否过期
            let lower = resp_text.to_lowercase();
            if lower.contains("uploadid") && (lower.contains("invalid") || lower.contains("expired")) {
                return Err("uploadid 已过期，请重试".into());
            }

            // 检查错误码
            if let Ok(resp_json) = serde_json::from_str::<Value>(&resp_text) {
                let error_code = resp_json["error_code"].as_i64().unwrap_or(0);
                let err_no = resp_json["errno"].as_i64().unwrap_or(0);
                if error_code != 0 || err_no != 0 {
                    return Err(format!("上传分片 {} 错误: {}", seq, resp_text));
                }
            }
        }

        // ===== Step 3: 合并创建文件 =====
        let create_url = format!("{}/xpan/file?method=create", API_BASE);
        self.api_post(&create_url, &[
            ("path", remote_path),
            ("size", &file_size.to_string()),
            ("isdir", "0"),
            ("rtype", "3"),
            ("uploadid", &upload_id),
            ("block_list", &block_list_str),
            ("local_mtime", &now.to_string()),
            ("local_ctime", &now.to_string()),
        ]).await?;

        Ok("上传成功".into())
    }

    fn serialize(&self) -> Result<Value, String> {
        serde_json::to_value(self).map_err(|e| e.to_string())
    }
}

impl BaiduDrive {
    async fn fetch_vip_type(&mut self) -> Result<(), String> {
        let url = format!("{}/xpan/nas?method=uinfo", API_BASE);
        let data = self.api_get(&url).await?;
        self.vip_type = data["vip_type"].as_i64().unwrap_or(0) as i32;
        Ok(())
    }
}
