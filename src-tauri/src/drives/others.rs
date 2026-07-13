// 夸克网盘、123网盘、天翼云盘、坚果云、OneDrive、Google Drive、PikPak、WebDAV 驱动
// 所有驱动共享 CloudDrive trait，按需加载到内存

use super::{CloudDrive, FileItem, Quota, UserInfo};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ==================== 通用基础实现 ====================

/// 通用 OAuth 驱动（适用于大多数使用标准 OAuth2 的网盘）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericOAuthDrive {
    drive_id: String,
    drive_type: String,
    api_key: String,
    secret_key: String,
    auth_url: String,
    token_url: String,
    api_base: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_at: i64,
}

impl GenericOAuthDrive {
    pub fn new(
        drive_id: &str, drive_type: &str,
        api_key: &str, secret_key: &str,
        auth_url: &str, token_url: &str, api_base: &str,
    ) -> Self {
        Self {
            drive_id: drive_id.to_string(),
            drive_type: drive_type.to_string(),
            api_key: api_key.to_string(),
            secret_key: secret_key.to_string(),
            auth_url: auth_url.to_string(),
            token_url: token_url.to_string(),
            api_base: api_base.to_string(),
            access_token: None,
            refresh_token: None,
            expires_at: 0,
        }
    }

    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap()
    }
}

#[async_trait]
impl CloudDrive for GenericOAuthDrive {
    fn drive_type(&self) -> &str { &self.drive_type }
    fn drive_id(&self) -> &str { &self.drive_id }

    fn get_auth_url(&self) -> String {
        match self.drive_type.as_str() {
            "quark" => format!("https://open-api.quark.cn/oauth/authorize?client_id={}&redirect_uri=oob&response_type=code", self.api_key),
            "onetwothree" => format!("https://www.123pan.com/oauth/authorize?client_id={}&redirect_uri=oob", self.api_key),
            "tianyi" => format!("https://open.189.cn/oauth2/authorize?app_id={}&redirect_uri=oob&response_type=code", self.api_key),
            "onedrive" => format!("https://login.microsoftonline.com/common/oauth2/v2.0/authorize?client_id={}&redirect_uri=oob&response_type=code&scope=Files.ReadWrite.All offline_access", self.api_key),
            "gdrive" => format!("https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri=oob&response_type=code&scope=https://www.googleapis.com/auth/drive", self.api_key),
            "pikpak" => format!("https://user.mypikpak.com/oauth/authorize?client_id={}&redirect_uri=oob", self.api_key),
            _ => format!("{}?client_id={}&redirect_uri=oob", self.auth_url, self.api_key),
        }
    }

    async fn exchange_token(&mut self, code: &str) -> Result<(), String> {
        let client = Self::client();
        let resp = match self.drive_type.as_str() {
            "onedrive" => {
                client.post(&self.token_url)
                    .form(&[
                        ("grant_type", "authorization_code"),
                        ("code", code),
                        ("client_id", &self.api_key),
                        ("client_secret", &self.secret_key),
                        ("redirect_uri", "oob"),
                    ])
                    .send().await
            }
            _ => {
                client.post(&self.token_url)
                    .json(&json!({
                        "client_id": self.api_key,
                        "client_secret": self.secret_key,
                        "grant_type": "authorization_code",
                        "code": code,
                    }))
                    .send().await
            }
        };

        let resp = resp.map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| format!("解析失败: {}", e))?;

        if let Some(token) = data["access_token"].as_str() {
            self.access_token = Some(token.to_string());
            self.refresh_token = data["refresh_token"].as_str().map(|s| s.to_string());
            let expires_in = data["expires_in"].as_i64().unwrap_or(7200);
            self.expires_at = chrono::Utc::now().timestamp() + expires_in;
            Ok(())
        } else {
            Err(format!("授权失败: {}", data))
        }
    }

    async fn refresh_token(&mut self) -> Result<(), String> {
        let rt = self.refresh_token.as_ref().ok_or("无 refresh_token")?.clone();
        let client = Self::client();
        let resp = client.post(&self.token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", rt.as_str()),
                ("client_id", self.api_key.as_str()),
                ("client_secret", self.secret_key.as_str()),
            ])
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| e.to_string())?;
        if let Some(token) = data["access_token"].as_str() {
            self.access_token = Some(token.to_string());
            self.refresh_token = data["refresh_token"].as_str().map(|s| s.to_string());
            let expires_in = data["expires_in"].as_i64().unwrap_or(7200);
            self.expires_at = chrono::Utc::now().timestamp() + expires_in;
            Ok(())
        } else {
            Err("刷新失败".into())
        }
    }

    fn is_logged_in(&self) -> bool {
        self.access_token.is_some() && chrono::Utc::now().timestamp() < self.expires_at
    }

    async fn get_user_info(&mut self) -> Result<UserInfo, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let client = Self::client();

        // 不同网盘的用户信息接口不同
        let (url, name_field) = match self.drive_type.as_str() {
            "quark" => (
                format!("{}/member", self.api_base),
                "data.nickname",
            ),
            "onetwothree" => (
                format!("{}/user/info", self.api_base),
                "data.nickname",
            ),
            "onedrive" => (
                "https://graph.microsoft.com/v1.0/me".to_string(),
                "displayName",
            ),
            "gdrive" => (
                "https://www.googleapis.com/drive/v3/about?fields=user".to_string(),
                "user.displayName",
            ),
            "pikpak" => (
                format!("{}/user/me", self.api_base),
                "nickname",
            ),
            _ => (
                format!("{}/user/info", self.api_base),
                "nickname",
            ),
        };

        let resp = client.get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| e.to_string())?;

        // 简单提取用户名（支持嵌套字段）
        let name = name_field.split('.').fold(&data, |acc, key| &acc[key])
            .as_str().unwrap_or("用户").to_string();

        Ok(UserInfo {
            name,
            avatar: None,
            vip_type: None,
            drive_type: self.drive_type.clone(),
            drive_id: self.drive_id.clone(),
        })
    }

    async fn get_quota(&mut self) -> Result<Quota, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let client = Self::client();

        let url = match self.drive_type.as_str() {
            "quark" => format!("{}/member/capacity", self.api_base),
            "onetwothree" => format!("{}/user/size", self.api_base),
            "onedrive" => "https://graph.microsoft.com/v1.0/me/drive".to_string(),
            "gdrive" => "https://www.googleapis.com/drive/v3/about?fields=storageQuota".to_string(),
            "pikpak" => format!("{}/user/vip", self.api_base),
            _ => format!("{}/user/quota", self.api_base),
        };

        let resp = client.get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| e.to_string())?;

        // 不同网盘的配额字段不同，这里做通用提取
        let (total, used) = match self.drive_type.as_str() {
            "onedrive" => {
                let t = data["quota"]["total"].as_u64().unwrap_or(0);
                let u = data["quota"]["used"].as_u64().unwrap_or(0);
                (t, u)
            }
            "gdrive" => {
                let t = data["storageQuota"]["limit"].as_str()
                    .and_then(|s| s.parse().ok()).unwrap_or(0u64);
                let u = data["storageQuota"]["usage"].as_str()
                    .and_then(|s| s.parse().ok()).unwrap_or(0u64);
                (t, u)
            }
            _ => {
                let t = data["total"].as_u64()
                    .or_else(|| data["data"]["total"].as_u64())
                    .unwrap_or(0);
                let u = data["used"].as_u64()
                    .or_else(|| data["data"]["used"].as_u64())
                    .unwrap_or(0);
                (t, u)
            }
        };

        Ok(Quota { total, used, free: total.saturating_sub(used) })
    }

    async fn list_files(&mut self, path: &str) -> Result<Vec<FileItem>, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let client = Self::client();

        // 根据不同网盘类型调用不同的 API
        let (url, list_field, id_field, name_field) = match self.drive_type.as_str() {
            "quark" => (
                format!("{}/file/sort?parent_fid={}&limit=200", self.api_base, path),
                "data.list", "fid", "file_name",
            ),
            "onetwothree" => (
                format!("{}/file/list?ParentFileId={}&Limit=200", self.api_base, path),
                "data.FileList", "FileId", "FileName",
            ),
            "onedrive" => {
                let folder = if path == "/" || path.is_empty() { "root" } else { path };
                (
                    format!("https://graph.microsoft.com/v1.0/me/drive/items/{}/children?$top=200", folder),
                    "value", "id", "name",
                )
            }
            "gdrive" => {
                let folder = if path == "/" || path.is_empty() { "root" } else { path };
                (
                    format!("https://www.googleapis.com/drive/v3/files?q='{}'+in+parents&pageSize=200&fields=files(id,name,mimeType,size,modifiedTime,thumbnailLink)", folder),
                    "files", "id", "name",
                )
            }
            "pikpak" => (
                format!("{}/file/list?parent_id={}&limit=200", self.api_base, path),
                "files", "id", "name",
            ),
            _ => (
                format!("{}/file/list?parent_id={}&limit=200", self.api_base, path),
                "list", "id", "name",
            ),
        };

        let resp = client.get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| e.to_string())?;

        // 提取列表
        let items = list_field.split('.').fold(&data, |acc, key| &acc[key])
            .as_array().cloned().unwrap_or_default();

        let did = self.drive_id.clone();
        let dt = self.drive_type.clone();
        Ok(items.iter().map(|item| {
            let id = item[id_field].as_str().unwrap_or("").to_string();
            let name = item[name_field].as_str().unwrap_or("").to_string();
            let size = item["size"].as_u64().unwrap_or(0);

            let is_dir = match self.drive_type.as_str() {
                "onedrive" => item["folder"].is_object(),
                "gdrive" => item["mimeType"].as_str().unwrap_or("").contains("folder"),
                _ => item["is_dir"].as_i64().unwrap_or(0) == 1
                    || item["type"].as_str() == Some("folder")
                    || item["kind"].as_str() == Some("drive#folder"),
            };

            let category = if is_dir { "folder" } else {
                let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
                match ext.as_str() {
                    "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "ts" => "video",
                    "mp3" | "flac" | "wav" | "aac" | "ogg" | "wma" => "audio",
                    "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "heic" => "image",
                    "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" => "document",
                    "apk" | "exe" | "msi" | "deb" | "rpm" | "dmg" => "app",
                    "zip" | "rar" | "7z" | "tar" | "gz" => "archive",
                    _ => "other",
                }
            };

            FileItem {
                id: id.clone(),
                name,
                path: id,
                size,
                is_dir,
                modified: item["updated_at"].as_i64()
                    .or_else(|| item["modified_time"].as_i64())
                    .unwrap_or(0),
                created: item["created_at"].as_i64().unwrap_or(0),
                category: category.to_string(),
                thumbnail: item["thumbnail"].as_str()
                    .or_else(|| item["thumbnailLink"].as_str())
                    .map(|s| s.to_string()),
                drive_id: did.clone(),
                drive_type: dt.clone(),
            }
        }).collect())
    }

    async fn search_files(&mut self, keyword: &str) -> Result<Vec<FileItem>, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let client = Self::client();

        let url = match self.drive_type.as_str() {
            "onedrive" => format!(
                "https://graph.microsoft.com/v1.0/me/drive/root/search(q='{}')", keyword
            ),
            "gdrive" => format!(
                "https://www.googleapis.com/drive/v3/files?q=name+contains+'{}'&pageSize=100", keyword
            ),
            _ => format!("{}/file/search?key={}&limit=100", self.api_base, keyword),
        };

        let resp = client.get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| e.to_string())?;

        // 通用提取（简化版，实际各网盘需要适配）
        let items = data["list"].as_array()
            .or_else(|| data["value"].as_array())
            .or_else(|| data["files"].as_array())
            .cloned().unwrap_or_default();

        let did = self.drive_id.clone();
        let dt = self.drive_type.clone();
        Ok(items.iter().map(|item| FileItem {
            id: item["id"].as_str().unwrap_or("").to_string(),
            name: item["name"].as_str().unwrap_or("").to_string(),
            path: item["id"].as_str().unwrap_or("").to_string(),
            size: item["size"].as_u64().unwrap_or(0),
            is_dir: false,
            modified: 0, created: 0,
            category: "other".into(),
            thumbnail: None,
            drive_id: did.clone(),
            drive_type: dt.clone(),
        }).collect())
    }

    async fn get_download_link(&mut self, file_id: &str) -> Result<String, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let client = Self::client();

        let url = match self.drive_type.as_str() {
            "onedrive" => format!(
                "https://graph.microsoft.com/v1.0/me/drive/items/{}/content", file_id
            ),
            "gdrive" => format!(
                "https://www.googleapis.com/drive/v3/files/{}?alt=media", file_id
            ),
            _ => format!("{}/file/download?file_id={}", self.api_base, file_id),
        };

        let resp = client.get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send().await.map_err(|e| format!("网络错误: {}", e))?;

        // OneDrive 和 Google Drive 返回重定向
        if let Some(location) = resp.headers().get("location") {
            return Ok(location.to_str().unwrap_or("").to_string());
        }

        let data: Value = resp.json().await.map_err(|e| e.to_string())?;
        data["download_url"].as_str()
            .or_else(|| data["url"].as_str())
            .or_else(|| data["data"]["download_url"].as_str())
            .map(|s| s.to_string())
            .ok_or("无下载链接".into())
    }

    async fn delete_files(&mut self, file_ids: &[String]) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let client = Self::client();
        for fid in file_ids {
            let url = match self.drive_type.as_str() {
                "onedrive" => format!("https://graph.microsoft.com/v1.0/me/drive/items/{}", fid),
                "gdrive" => format!("https://www.googleapis.com/drive/v3/files/{}", fid),
                _ => format!("{}/file/delete?file_id={}", self.api_base, fid),
            };
            client.delete(&url)
                .header("Authorization", format!("Bearer {}", token))
                .send().await.map_err(|e| format!("网络错误: {}", e))?;
        }
        Ok(())
    }

    async fn create_folder(&mut self, path: &str) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let client = Self::client();
        let parts: Vec<&str> = path.rsplitn(2, '/').collect();
        let (name, parent) = if parts.len() == 2 { (parts[0], parts[1]) } else { (parts[0], "root") };

        let body = match self.drive_type.as_str() {
            "onedrive" => json!({ "name": name, "folder": {}, "@microsoft.graph.conflictBehavior": "rename" }),
            "gdrive" => json!({ "name": name, "mimeType": "application/vnd.google-apps.folder", "parents": [parent] }),
            _ => json!({ "parent_id": parent, "name": name, "type": "folder" }),
        };

        let url = match self.drive_type.as_str() {
            "onedrive" => format!("https://graph.microsoft.com/v1.0/me/drive/items/{}/children", parent),
            "gdrive" => "https://www.googleapis.com/drive/v3/files".to_string(),
            _ => format!("{}/file/create", self.api_base),
        };

        client.post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        Ok(())
    }

    async fn rename_file(&mut self, file_id: &str, new_name: &str) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let client = Self::client();

        let (url, body) = match self.drive_type.as_str() {
            "onedrive" => (
                format!("https://graph.microsoft.com/v1.0/me/drive/items/{}", file_id),
                json!({ "name": new_name }),
            ),
            "gdrive" => (
                format!("https://www.googleapis.com/drive/v3/files/{}", file_id),
                json!({ "name": new_name }),
            ),
            _ => (
                format!("{}/file/update?file_id={}", self.api_base, file_id),
                json!({ "name": new_name }),
            ),
        };

        client.patch(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        Ok(())
    }

    async fn upload_file(&mut self, _local_path: &str, _remote_path: &str) -> Result<String, String> {
        Err("上传功能开发中".into())
    }

    fn serialize(&self) -> Result<Value, String> {
        serde_json::to_value(self).map_err(|e| e.to_string())
    }
}

// ==================== WebDAV 通用驱动 ====================

/// WebDAV 驱动（坚果云、InfiniCLOUD 等支持 WebDAV 的网盘）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavDrive {
    drive_id: String,
    server_url: String,
    username: String,
    password: String,
}

impl WebDavDrive {
    pub fn new(drive_id: &str, server_url: &str, username: &str, password: &str) -> Self {
        Self {
            drive_id: drive_id.to_string(),
            server_url: server_url.trim_end_matches('/').to_string(),
            username: username.to_string(),
            password: password.to_string(),
        }
    }

    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap()
    }
}

#[async_trait]
impl CloudDrive for WebDavDrive {
    fn drive_type(&self) -> &str { "webdav" }
    fn drive_id(&self) -> &str { &self.drive_id }
    fn get_auth_url(&self) -> String { String::new() } // WebDAV 不需要 OAuth

    async fn exchange_token(&mut self, _code: &str) -> Result<(), String> {
        // WebDAV 使用 Basic Auth，不需要 token 交换
        // 验证连接是否成功
        let client = Self::client();
        let resp = client.request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &self.server_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "0")
            .body("<?xml version=\"1.0\"?><d:propfind xmlns:d=\"DAV:\"><d:prop><d:displayname/></d:prop></d:propfind>")
            .send().await
            .map_err(|e| format!("连接失败: {}", e))?;
        if resp.status().is_success() || resp.status().as_u16() == 207 {
            Ok(())
        } else {
            Err(format!("认证失败 (HTTP {})", resp.status()))
        }
    }

    async fn refresh_token(&mut self) -> Result<(), String> { Ok(()) }
    fn is_logged_in(&self) -> bool { !self.username.is_empty() }

    async fn get_user_info(&mut self) -> Result<UserInfo, String> {
        Ok(UserInfo {
            name: self.username.clone(),
            avatar: None,
            vip_type: None,
            drive_type: "webdav".into(),
            drive_id: self.drive_id.clone(),
        })
    }

    async fn get_quota(&mut self) -> Result<Quota, String> {
        let client = Self::client();
        let resp = client.request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &self.server_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "0")
            .body("<?xml version=\"1.0\"?><d:propfind xmlns:d=\"DAV:\"><d:prop><d:quota-available-bytes/><d:quota-used-bytes/></d:prop></d:propfind>")
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let body = resp.text().await.map_err(|e| e.to_string())?;

        // 简单解析 XML（生产环境应该用 xml-rs）
        let free = extract_xml_value(&body, "quota-available-bytes").unwrap_or(0);
        let used = extract_xml_value(&body, "quota-used-bytes").unwrap_or(0);
        Ok(Quota { total: free + used, used, free })
    }

    async fn list_files(&mut self, path: &str) -> Result<Vec<FileItem>, String> {
        let client = Self::client();
        let url = format!("{}/{}", self.server_url, path.trim_start_matches('/'));
        let resp = client.request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "1")
            .body("<?xml version=\"1.0\"?><d:propfind xmlns:d=\"DAV:\"><d:prop><d:displayname/><d:getcontentlength/><d:getlastmodified/><d:resourcetype/></d:prop></d:propfind>")
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let body = resp.text().await.map_err(|e| e.to_string())?;

        // 简单解析（生产环境应该用 xml-rs）
        let did = self.drive_id.clone();
        let mut items = Vec::new();
        for response in body.split("<d:response>").skip(1) {
            let href = extract_xml_text(response, "href").unwrap_or_default();
            let name = href.rsplit('/').find(|s| !s.is_empty()).unwrap_or("").to_string();
            let size = extract_xml_text(response, "getcontentlength")
                .and_then(|s| s.parse().ok()).unwrap_or(0u64);
            let is_dir = response.contains("<d:collection");

            let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
            let category = if is_dir { "folder" } else {
                match ext.as_str() {
                    "mp4" | "mkv" | "avi" | "mov" => "video",
                    "mp3" | "flac" | "wav" => "audio",
                    "jpg" | "jpeg" | "png" | "gif" => "image",
                    "pdf" | "doc" | "docx" | "txt" => "document",
                    _ => "other",
                }
            };

            if !name.is_empty() {
                items.push(FileItem {
                    id: href.clone(),
                    name,
                    path: href,
                    size,
                    is_dir,
                    modified: 0, created: 0,
                    category: category.to_string(),
                    thumbnail: None,
                    drive_id: did.clone(),
                    drive_type: "webdav".into(),
                });
            }
        }
        Ok(items)
    }

    async fn search_files(&mut self, _keyword: &str) -> Result<Vec<FileItem>, String> {
        Err("WebDAV 不支持搜索，请在目录下浏览".into())
    }

    async fn get_download_link(&mut self, file_path: &str) -> Result<String, String> {
        // WebDAV 下载链接就是文件路径
        Ok(format!("{}/{}", self.server_url, file_path.trim_start_matches('/')))
    }

    async fn delete_files(&mut self, paths: &[String]) -> Result<(), String> {
        let client = Self::client();
        for path in paths {
            let url = format!("{}/{}", self.server_url, path.trim_start_matches('/'));
            client.delete(&url)
                .basic_auth(&self.username, Some(&self.password))
                .send().await.map_err(|e| format!("网络错误: {}", e))?;
        }
        Ok(())
    }

    async fn create_folder(&mut self, path: &str) -> Result<(), String> {
        let client = Self::client();
        let url = format!("{}/{}", self.server_url, path.trim_start_matches('/'));
        client.request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &url)
            .basic_auth(&self.username, Some(&self.password))
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        Ok(())
    }

    async fn rename_file(&mut self, path: &str, new_name: &str) -> Result<(), String> {
        let client = Self::client();
        let parent = &path[..path.rfind('/').unwrap_or(0)];
        let new_url = format!("{}/{}/{}", self.server_url, parent.trim_start_matches('/'), new_name);
        let old_url = format!("{}/{}", self.server_url, path.trim_start_matches('/'));
        client.request(reqwest::Method::from_bytes(b"MOVE").unwrap(), &old_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Destination", &new_url)
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        Ok(())
    }

    async fn upload_file(&mut self, _local_path: &str, _remote_path: &str) -> Result<String, String> {
        Err("上传功能开发中".into())
    }

    fn serialize(&self) -> Result<Value, String> {
        serde_json::to_value(self).map_err(|e| e.to_string())
    }
}

// ==================== 辅助函数 ====================

fn extract_xml_value(xml: &str, tag: &str) -> Option<u64> {
    let start = format!("<d:{}>", tag);
    let end = format!("</d:{}>", tag);
    if let Some(s) = xml.find(&start) {
        let rest = &xml[s + start.len()..];
        if let Some(e) = rest.find(&end) {
            return rest[..e].trim().parse().ok();
        }
    }
    None
}

fn extract_xml_text(xml: &str, tag: &str) -> Option<String> {
    let patterns = [
        (format!("<d:{}>", tag), format!("</d:{}>", tag)),
        (format!("<D:{}>", tag), format!("</D:{}>", tag)),
        (format!("<{}>", tag), format!("</{}>", tag)),
    ];
    for (start, end) in &patterns {
        if let Some(s) = xml.find(start.as_str()) {
            let rest = &xml[s + start.len()..];
            if let Some(e) = rest.find(end.as_str()) {
                return Some(rest[..e].trim().to_string());
            }
        }
    }
    None
}
