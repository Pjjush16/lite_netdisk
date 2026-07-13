// 阿里云盘驱动

use super::{CloudDrive, FileItem, Quota, UserInfo};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const AUTH_URL: &str = "https://open.aliyundrive.com/oauth/authorize";
const TOKEN_URL: &str = "https://open.aliyundrive.com/oauth/access_token";
const API_BASE: &str = "https://open.aliyundrive.com/adrive/v1.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliyunDrive {
    drive_id: String,
    client_id: String,
    client_secret: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_at: i64,
    user_drive_id: Option<String>,
}

impl AliyunDrive {
    pub fn new(drive_id: &str, client_id: &str, client_secret: &str) -> Self {
        Self {
            drive_id: drive_id.to_string(),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            access_token: None,
            refresh_token: None,
            expires_at: 0,
            user_drive_id: None,
        }
    }

    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap()
    }

    fn auth_header(&self) -> Result<Vec<(String, String)>, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        Ok(vec![("Authorization".into(), format!("Bearer {}", token))])
    }
}

#[async_trait]
impl CloudDrive for AliyunDrive {
    fn drive_type(&self) -> &str { "aliyun" }
    fn drive_id(&self) -> &str { &self.drive_id }

    fn get_auth_url(&self) -> String {
        format!("{}?client_id={}&redirect_uri=oob&scope=user:base,file:all:read,file:all:write",
            AUTH_URL, self.client_id)
    }

    async fn exchange_token(&mut self, code: &str) -> Result<(), String> {
        let client = Self::client();
        let body = json!({
            "client_id": self.client_id,
            "client_secret": self.client_secret,
            "grant_type": "authorization_code",
            "code": code,
        });
        let resp = client.post(TOKEN_URL)
            .json(&body)
            .send().await
            .map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| format!("解析失败: {}", e))?;

        if let Some(token) = data["access_token"].as_str() {
            self.access_token = Some(token.to_string());
            self.refresh_token = data["refresh_token"].as_str().map(|s| s.to_string());
            let expires_in = data["expires_in"].as_i64().unwrap_or(7200);
            self.expires_at = chrono::Utc::now().timestamp() + expires_in;

            // 获取用户的 drive_id
            let headers = self.auth_header()?;
            let resp2 = client.get("https://open.aliyundrive.com/adrive/v1.0/user/getDriveInfo")
                .header("Authorization", format!("Bearer {}", token))
                .send().await
                .map_err(|e| format!("获取 drive_id 失败: {}", e))?;
            let data2: Value = resp2.json().await.map_err(|e| e.to_string())?;
            self.user_drive_id = data2["default_drive_id"].as_i64().map(|v| v.to_string());

            Ok(())
        } else {
            Err(format!("授权失败: {}", data["message"].as_str().unwrap_or("未知")))
        }
    }

    async fn refresh_token(&mut self) -> Result<(), String> {
        let rt = self.refresh_token.as_ref().ok_or("无 refresh_token")?.clone();
        let client = Self::client();
        let body = json!({
            "client_id": self.client_id,
            "client_secret": self.client_secret,
            "grant_type": "refresh_token",
            "refresh_token": rt,
        });
        let resp = client.post(TOKEN_URL).json(&body).send().await
            .map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| format!("解析失败: {}", e))?;
        if let Some(token) = data["access_token"].as_str() {
            self.access_token = Some(token.to_string());
            self.refresh_token = data["refresh_token"].as_str().map(|s| s.to_string());
            let expires_in = data["expires_in"].as_i64().unwrap_or(7200);
            self.expires_at = chrono::Utc::now().timestamp() + expires_in;
            Ok(())
        } else {
            Err("刷新 token 失败".into())
        }
    }

    fn is_logged_in(&self) -> bool {
        self.access_token.is_some() && chrono::Utc::now().timestamp() < self.expires_at
    }

    async fn get_user_info(&mut self) -> Result<UserInfo, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let client = Self::client();
        let resp = client.get("https://open.aliyundrive.com/adrive/v1.0/user/getDriveInfo")
            .header("Authorization", format!("Bearer {}", token))
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| e.to_string())?;

        Ok(UserInfo {
            name: data["name"].as_str().unwrap_or("阿里云盘用户").to_string(),
            avatar: data["avatar"].as_str().map(|s| s.to_string()),
            vip_type: None,
            drive_type: "aliyun".into(),
            drive_id: self.drive_id.clone(),
        })
    }

    async fn get_quota(&mut self) -> Result<Quota, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let client = Self::client();
        let resp = client.post("https://open.aliyundrive.com/adrive/v1.0/user/getSpaceInfo")
            .header("Authorization", format!("Bearer {}", token))
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| e.to_string())?;

        let total = data["personal_space_info"]["total_size"].as_u64().unwrap_or(0);
        let used = data["personal_space_info"]["used_size"].as_u64().unwrap_or(0);
        Ok(Quota { total, used, free: total.saturating_sub(used) })
    }

    async fn list_files(&mut self, parent_file_id: &str) -> Result<Vec<FileItem>, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let drive_id = self.user_drive_id.as_ref().ok_or("未获取 drive_id")?;

        // "root" 表示根目录
        let parent_id = if parent_file_id == "/" || parent_file_id.is_empty() { "root" } else { parent_file_id };

        let client = Self::client();
        let body = json!({
            "drive_id": drive_id,
            "parent_file_id": parent_id,
            "limit": 200,
            "order_by": "name",
            "order_direction": "ASC"
        });

        let resp = client.post(&format!("{}/openFile/list", API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| e.to_string())?;

        let items = data["items"].as_array().cloned().unwrap_or_default();
        let did = self.drive_id.clone();
        Ok(items.iter().map(|item| {
            let is_dir = item["type"].as_str() == Some("folder");
            let cat = item["file_extension"].as_str().unwrap_or("");
            let category = if is_dir { "folder" } else {
                match cat {
                    "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" => "video",
                    "mp3" | "flac" | "wav" | "aac" | "ogg" => "audio",
                    "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" => "image",
                    "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" => "document",
                    _ => "other",
                }
            };

            FileItem {
                id: item["file_id"].as_str().unwrap_or("").to_string(),
                name: item["name"].as_str().unwrap_or("").to_string(),
                path: item["file_id"].as_str().unwrap_or("").to_string(),
                size: item["size"].as_u64().unwrap_or(0),
                is_dir,
                modified: chrono::DateTime::parse_from_rfc3339(
                    item["updated_at"].as_str().unwrap_or("1970-01-01T00:00:00Z")
                ).map(|dt| dt.timestamp()).unwrap_or(0),
                created: chrono::DateTime::parse_from_rfc3339(
                    item["created_at"].as_str().unwrap_or("1970-01-01T00:00:00Z")
                ).map(|dt| dt.timestamp()).unwrap_or(0),
                category: category.to_string(),
                thumbnail: item["thumbnail"].as_str().map(|s| s.to_string()),
                drive_id: did.clone(),
                drive_type: "aliyun".into(),
            }
        }).collect())
    }

    async fn search_files(&mut self, keyword: &str) -> Result<Vec<FileItem>, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let drive_id = self.user_drive_id.as_ref().ok_or("未获取 drive_id")?;
        let client = Self::client();
        let body = json!({
            "drive_id": drive_id,
            "query": format!("name match \"{}\"", keyword),
            "limit": 100,
        });
        let resp = client.post(&format!("{}/openFile/search", API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| e.to_string())?;

        let items = data["items"].as_array().cloned().unwrap_or_default();
        let did = self.drive_id.clone();
        Ok(items.iter().map(|item| {
            let is_dir = item["type"].as_str() == Some("folder");
            FileItem {
                id: item["file_id"].as_str().unwrap_or("").to_string(),
                name: item["name"].as_str().unwrap_or("").to_string(),
                path: item["file_id"].as_str().unwrap_or("").to_string(),
                size: item["size"].as_u64().unwrap_or(0),
                is_dir,
                modified: 0,
                created: 0,
                category: if is_dir { "folder".into() } else { "other".into() },
                thumbnail: item["thumbnail"].as_str().map(|s| s.to_string()),
                drive_id: did.clone(),
                drive_type: "aliyun".into(),
            }
        }).collect())
    }

    async fn get_download_link(&mut self, file_id: &str) -> Result<String, String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let drive_id = self.user_drive_id.as_ref().ok_or("未获取 drive_id")?;
        let client = Self::client();
        let body = json!({ "drive_id": drive_id, "file_id": file_id });
        let resp = client.post(&format!("{}/openFile/getDownloadUrl", API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        let data: Value = resp.json().await.map_err(|e| e.to_string())?;
        data["url"].as_str().map(|s| s.to_string()).ok_or("无下载链接".into())
    }

    async fn delete_files(&mut self, file_ids: &[String]) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let drive_id = self.user_drive_id.as_ref().ok_or("未获取 drive_id")?;
        let client = Self::client();
        for fid in file_ids {
            let body = json!({ "drive_id": drive_id, "file_id": fid });
            client.post(&format!("{}/openFile/delete", API_BASE))
                .header("Authorization", format!("Bearer {}", token))
                .json(&body)
                .send().await.map_err(|e| format!("网络错误: {}", e))?;
        }
        Ok(())
    }

    async fn create_folder(&mut self, parent_and_name: &str) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let drive_id = self.user_drive_id.as_ref().ok_or("未获取 drive_id")?;
        // parent_and_name 格式: "parent_id/name"
        let parts: Vec<&str> = parent_and_name.rsplitn(2, '/').collect();
        let (name, parent_id) = if parts.len() == 2 { (parts[0], parts[1]) } else { (parts[0], "root") };
        let client = Self::client();
        let body = json!({
            "drive_id": drive_id,
            "parent_file_id": parent_id,
            "name": name,
            "type": "folder",
        });
        client.post(&format!("{}/openFile/create", API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        Ok(())
    }

    async fn rename_file(&mut self, file_id: &str, new_name: &str) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("未登录")?;
        let drive_id = self.user_drive_id.as_ref().ok_or("未获取 drive_id")?;
        let client = Self::client();
        let body = json!({ "drive_id": drive_id, "file_id": file_id, "name": new_name });
        client.post(&format!("{}/openFile/update", API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send().await.map_err(|e| format!("网络错误: {}", e))?;
        Ok(())
    }

    async fn upload_file(&mut self, _local_path: &str, _remote_path: &str) -> Result<String, String> {
        // 阿里云上传需要获取上传凭证 + 分片上传
        // TODO: 实现完整上传流程
        Err("上传功能开发中".into())
    }

    fn serialize(&self) -> Result<Value, String> {
        serde_json::to_value(self).map_err(|e| e.to_string())
    }
}
