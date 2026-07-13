// 网盘驱动管理器

pub mod baidu;
pub mod aliyun;
pub mod others;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 文件条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub modified: i64,
    pub created: i64,
    pub category: String,
    pub thumbnail: Option<String>,
    pub drive_id: String,
    pub drive_type: String,
}

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub avatar: Option<String>,
    pub vip_type: Option<String>,
    pub drive_type: String,
    pub drive_id: String,
}

/// 网盘配额
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quota {
    pub total: u64,
    pub used: u64,
    pub free: u64,
}

/// 网盘驱动 trait - 所有方法用 &mut self 以支持自动 token 刷新
#[async_trait]
pub trait CloudDrive: Send + Sync {
    /// 驱动类型名
    fn drive_type(&self) -> &str;

    /// 驱动 ID
    fn drive_id(&self) -> &str;

    /// 获取 OAuth 授权 URL
    fn get_auth_url(&self) -> String;

    /// 用授权码换取 token
    async fn exchange_token(&mut self, code: &str) -> Result<(), String>;

    /// 刷新 token
    async fn refresh_token(&mut self) -> Result<(), String>;

    /// 是否已登录
    fn is_logged_in(&self) -> bool;

    /// 获取用户信息
    async fn get_user_info(&mut self) -> Result<UserInfo, String>;

    /// 获取配额
    async fn get_quota(&mut self) -> Result<Quota, String>;

    /// 列出文件
    async fn list_files(&mut self, path: &str) -> Result<Vec<FileItem>, String>;

    /// 搜索文件
    async fn search_files(&mut self, keyword: &str) -> Result<Vec<FileItem>, String>;

    /// 获取下载链接
    async fn get_download_link(&mut self, file_id: &str) -> Result<String, String>;

    /// 删除文件
    async fn delete_files(&mut self, paths: &[String]) -> Result<(), String>;

    /// 创建文件夹
    async fn create_folder(&mut self, path: &str) -> Result<(), String>;

    /// 重命名
    async fn rename_file(&mut self, path: &str, new_name: &str) -> Result<(), String>;

    /// 上传文件（返回任务 ID）
    async fn upload_file(&mut self, local_path: &str, remote_path: &str) -> Result<String, String>;

    /// 序列化为 JSON（用于持久化）
    fn serialize(&self) -> Result<serde_json::Value, String>;
}

/// 下载任务状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub save_path: String,
    pub total_size: u64,
    pub downloaded: u64,
    pub status: String, // "downloading", "paused", "completed", "failed", "cancelled"
    pub error: Option<String>,
    pub drive_type: String,
    pub created_at: i64,
}

/// 网盘驱动管理器
pub struct DriveManager {
    drives: HashMap<String, Arc<RwLock<Box<dyn CloudDrive>>>>,
}

impl DriveManager {
    pub fn new() -> Self {
        Self {
            drives: HashMap::new(),
        }
    }

    /// 添加网盘驱动
    pub async fn add_drive(&mut self, drive: Box<dyn CloudDrive>) {
        let id = drive.drive_id().to_string();
        self.drives.insert(id, Arc::new(RwLock::new(drive)));
    }

    /// 移除网盘驱动
    pub async fn remove_drive(&mut self, drive_id: &str) {
        self.drives.remove(drive_id);
    }

    /// 获取所有已连接的网盘
    pub async fn get_connected_drives(&self) -> Vec<(String, String, bool)> {
        let mut result = Vec::new();
        for (id, drive) in &self.drives {
            let d = drive.read().await;
            result.push((id.clone(), d.drive_type().to_string(), d.is_logged_in()));
        }
        result
    }

    /// 获取指定驱动
    pub fn get_drive(&self, drive_id: &str) -> Option<Arc<RwLock<Box<dyn CloudDrive>>>> {
        self.drives.get(drive_id).cloned()
    }

    /// 获取所有驱动（用于统一搜索）
    pub fn get_all_drives(&self) -> Vec<(String, Arc<RwLock<Box<dyn CloudDrive>>>)> {
        self.drives.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

/// 下载管理器
pub struct DownloadManager {
    tasks: HashMap<String, DownloadTask>,
    cancel_tokens: HashMap<String, tokio::sync::oneshot::Sender<()>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            cancel_tokens: HashMap::new(),
        }
    }

    pub fn add_task(&mut self, task: DownloadTask) {
        self.tasks.insert(task.id.clone(), task);
    }

    pub fn get_tasks(&self) -> Vec<DownloadTask> {
        self.tasks.values().cloned().collect()
    }

    pub fn get_task(&self, id: &str) -> Option<&DownloadTask> {
        self.tasks.get(id)
    }

    pub fn update_task(&mut self, id: &str, downloaded: u64, status: &str) {
        if let Some(task) = self.tasks.get_mut(id) {
            task.downloaded = downloaded;
            task.status = status.to_string();
        }
    }

    pub fn cancel_task(&mut self, id: &str) -> bool {
        if let Some(tx) = self.cancel_tokens.remove(id) {
            let _ = tx.send(());
            if let Some(task) = self.tasks.get_mut(id) {
                task.status = "cancelled".to_string();
            }
            true
        } else {
            false
        }
    }

    pub fn set_cancel_token(&mut self, id: String, tx: tokio::sync::oneshot::Sender<()>) {
        self.cancel_tokens.insert(id, tx);
    }
}
