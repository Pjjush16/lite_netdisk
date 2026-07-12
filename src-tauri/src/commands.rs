// Tauri 命令 - 前端通过 invoke() 调用

use crate::AppState;
use crate::drives::{
    FileItem, UserInfo, Quota, DownloadTask,
    baidu::BaiduDrive,
    aliyun::AliyunDrive,
    others::{GenericOAuthDrive, WebDavDrive},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

// ======================== 认证相关 ========================

#[tauri::command]
pub async fn get_auth_url(
    state: State<'_, AppState>,
    drive_id: String,
) -> Result<String, String> {
    let dm = state.drive_manager.read().await;
    let drive = dm.get_drive(&drive_id).ok_or("网盘未添加")?;
    let mut d = drive.write().await;
    Ok(d.get_auth_url())
}

#[tauri::command]
pub async fn exchange_token(
    state: State<'_, AppState>,
    drive_id: String,
    code: String,
) -> Result<(), String> {
    let dm = state.drive_manager.read().await;
    let drive = dm.get_drive(&drive_id).ok_or("网盘未添加")?;
    let mut d = drive.write().await;
    d.exchange_token(&code).await
}

#[tauri::command]
pub async fn get_user_info(
    state: State<'_, AppState>,
    drive_id: String,
) -> Result<UserInfo, String> {
    let dm = state.drive_manager.read().await;
    let drive = dm.get_drive(&drive_id).ok_or("网盘未添加")?;
    let mut d = drive.write().await;
    d.get_user_info().await
}

#[tauri::command]
pub async fn logout(
    state: State<'_, AppState>,
    drive_id: String,
) -> Result<(), String> {
    // 清除 token（通过重新创建驱动实例实现）
    let _dm = state.drive_manager.read().await;
    // TODO: 实现具体的 logout 逻辑
    Ok(())
}

// ======================== 文件操作 ========================

#[tauri::command]
pub async fn list_files(
    state: State<'_, AppState>,
    drive_id: String,
    path: String,
) -> Result<Vec<FileItem>, String> {
    let dm = state.drive_manager.read().await;
    let drive = dm.get_drive(&drive_id).ok_or("网盘未添加")?;
    let mut d = drive.write().await;
    d.list_files(&path).await
}

#[tauri::command]
pub async fn search_files(
    state: State<'_, AppState>,
    drive_id: String,
    keyword: String,
) -> Result<Vec<FileItem>, String> {
    let dm = state.drive_manager.read().await;

    // 如果指定了 drive_id，只搜索该网盘
    if !drive_id.is_empty() && drive_id != "all" {
        let drive = dm.get_drive(&drive_id).ok_or("网盘未添加")?;
        let mut d = drive.write().await;
        return d.search_files(&keyword).await;
    }

    // 跨盘搜索：搜索所有已登录的网盘
    let drives = dm.get_all_drives();
    let mut all_results: Vec<FileItem> = Vec::new();

    for (_id, drive) in &drives {
        let mut d = drive.write().await;
        if d.is_logged_in() {
            match d.search_files(&keyword).await {
                Ok(items) => all_results.extend(items),
                Err(_) => {} // 某个网盘搜索失败不影响其他
            }
        }
    }

    // 按名称排序
    all_results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(all_results)
}

#[tauri::command]
pub async fn get_download_link(
    state: State<'_, AppState>,
    drive_id: String,
    file_id: String,
) -> Result<String, String> {
    let dm = state.drive_manager.read().await;
    let drive = dm.get_drive(&drive_id).ok_or("网盘未添加")?;
    let mut d = drive.write().await;
    d.get_download_link(&file_id).await
}

#[tauri::command]
pub async fn delete_files(
    state: State<'_, AppState>,
    drive_id: String,
    paths: Vec<String>,
) -> Result<(), String> {
    let dm = state.drive_manager.read().await;
    let drive = dm.get_drive(&drive_id).ok_or("网盘未添加")?;
    let mut d = drive.write().await;
    d.delete_files(&paths).await
}

#[tauri::command]
pub async fn create_folder(
    state: State<'_, AppState>,
    drive_id: String,
    path: String,
) -> Result<(), String> {
    let dm = state.drive_manager.read().await;
    let drive = dm.get_drive(&drive_id).ok_or("网盘未添加")?;
    let mut d = drive.write().await;
    d.create_folder(&path).await
}

#[tauri::command]
pub async fn rename_file(
    state: State<'_, AppState>,
    drive_id: String,
    path: String,
    new_name: String,
) -> Result<(), String> {
    let dm = state.drive_manager.read().await;
    let drive = dm.get_drive(&drive_id).ok_or("网盘未添加")?;
    let mut d = drive.write().await;
    d.rename_file(&path, &new_name).await
}

// ======================== 下载管理 ========================

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub drive_id: String,
    pub file_id: String,
    pub filename: String,
    pub save_path: String,
}

#[tauri::command]
pub async fn start_download(
    state: State<'_, AppState>,
    req: DownloadRequest,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let task_id = uuid::Uuid::new_v4().to_string();

    // 获取下载链接
    let dlink = {
        let dm = state.drive_manager.read().await;
        let drive = dm.get_drive(&req.drive_id).ok_or("网盘未添加")?;
        let mut d = drive.write().await;
        d.get_download_link(&req.file_id).await?
    };

    let task = DownloadTask {
        id: task_id.clone(),
        filename: req.filename.clone(),
        url: dlink.clone(),
        save_path: req.save_path.clone(),
        total_size: 0,
        downloaded: 0,
        status: "downloading".to_string(),
        error: None,
        drive_type: String::new(),
        created_at: chrono::Utc::now().timestamp(),
    };

    // 添加到任务列表
    {
        let mut dm = state.download_manager.write().await;
        dm.add_task(task);
    }

    // 创建取消通道
    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut dm = state.download_manager.write().await;
        dm.set_cancel_token(task_id.clone(), cancel_tx);
    }

    // 启动异步下载
    let task_id_clone = task_id.clone();
    let download_mgr = state.download_manager.clone();
    let app = app_handle.clone();

    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3600)) // 大文件下载 1 小时超时
            .build()
            .unwrap();

        // 获取文件大小
        let head_resp = match client.head(&dlink).send().await {
            Ok(r) => r,
            Err(e) => {
                let mut dm = download_mgr.write().await;
                dm.update_task(&task_id_clone, 0, &format!("failed:{}", e));
                return;
            }
        };

        let total_size = head_resp.headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        {
            let mut dm = download_mgr.write().await;
            if let Some(t) = dm.tasks.get_mut(&task_id_clone) {
                t.total_size = total_size;
            }
        }

        // 开始下载（支持断点续传）
        let save_path = std::path::PathBuf::from(&req.save_path);
        if let Some(parent) = save_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let existing_size = if save_path.exists() {
            tokio::fs::metadata(&save_path).await.map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let mut req_builder = client.get(&dlink);
        if existing_size > 0 && existing_size < total_size {
            req_builder = req_builder.header("Range", format!("bytes={}-", existing_size));
        }

        let resp = match req_builder.send().await {
            Ok(r) => r,
            Err(e) => {
                let mut dm = download_mgr.write().await;
                dm.update_task(&task_id_clone, 0, &format!("failed:{}", e));
                return;
            }
        };

        let mut file = match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&save_path)
            .await
        {
            Ok(f) => f,
            Err(e) => {
                let mut dm = download_mgr.write().await;
                dm.update_task(&task_id_clone, 0, &format!("failed:{}", e));
                return;
            }
        };

        use tokio::io::AsyncWriteExt;

        let mut downloaded = existing_size;
        let mut stream = resp.bytes_stream();

        use futures::StreamExt;
        loop {
            tokio::select! {
                chunk = stream.next() => {
                    match chunk {
                        Some(Ok(bytes)) => {
                            if file.write_all(&bytes).await.is_err() {
                                let mut dm = download_mgr.write().await;
                                dm.update_task(&task_id_clone, downloaded, "failed");
                                return;
                            }
                            downloaded += bytes.len() as u64;

                            // 更新进度
                            {
                                let mut dm = download_mgr.write().await;
                                dm.update_task(&task_id_clone, downloaded, "downloading");
                            }

                            // 通知前端更新进度
                            let _ = app.emit("download-progress", serde_json::json!({
                                "id": task_id_clone,
                                "downloaded": downloaded,
                                "total": total_size,
                            }));
                        }
                        Some(Err(e)) => {
                            let mut dm = download_mgr.write().await;
                            dm.update_task(&task_id_clone, downloaded, &format!("failed:{}", e));
                            return;
                        }
                        None => break, // 下载完成
                    }
                }
                _ = &mut cancel_rx => {
                    // 用户取消
                    let mut dm = download_mgr.write().await;
                    dm.update_task(&task_id_clone, downloaded, "cancelled");
                    return;
                }
            }
        }

        // 下载完成
        {
            let mut dm = download_mgr.write().await;
            dm.update_task(&task_id_clone, downloaded, "completed");
        }

        let _ = app.emit("download-progress", serde_json::json!({
            "id": task_id_clone,
            "downloaded": downloaded,
            "total": total_size,
            "status": "completed",
        }));
    });

    Ok(task_id)
}

#[tauri::command]
pub async fn cancel_download(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<bool, String> {
    let mut dm = state.download_manager.write().await;
    Ok(dm.cancel_task(&task_id))
}

#[tauri::command]
pub async fn get_download_list(
    state: State<'_, AppState>,
) -> Result<Vec<DownloadTask>, String> {
    let dm = state.download_manager.read().await;
    Ok(dm.get_tasks())
}

// ======================== 跨盘传输 ========================

#[derive(Debug, Serialize, Deserialize)]
pub struct TransferRequest {
    pub source_drive_id: String,
    pub source_file_id: String,
    pub target_drive_id: String,
    pub target_path: String,
    pub filename: String,
}

#[tauri::command]
pub async fn transfer_files(
    state: State<'_, AppState>,
    req: TransferRequest,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let transfer_id = uuid::Uuid::new_v4().to_string();

    // 获取源文件下载链接
    let dlink = {
        let dm = state.drive_manager.read().await;
        let drive = dm.get_drive(&req.source_drive_id).ok_or("源网盘未添加")?;
        let mut d = drive.write().await;
        d.get_download_link(&req.source_file_id).await?
    };

    let tid = transfer_id.clone();
    let dm = state.drive_manager.clone();
    let app = app_handle.clone();

    // 启动流式传输：从源下载 -> 直接上传到目标
    tokio::spawn(async move {
        let client = reqwest::Client::new();

        // 下载源文件
        let resp = match client.get(&dlink).send().await {
            Ok(r) => r,
            Err(e) => {
                let _ = app.emit("transfer-error", serde_json::json!({
                    "id": tid,
                    "error": format!("下载失败: {}", e),
                }));
                return;
            }
        };

        let total = resp.content_length().unwrap_or(0);
        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                let _ = app.emit("transfer-error", serde_json::json!({
                    "id": tid,
                    "error": format!("读取失败: {}", e),
                }));
                return;
            }
        };

        let _ = app.emit("transfer-progress", serde_json::json!({
            "id": tid,
            "phase": "downloaded",
            "size": bytes.len(),
        }));

        // 上传到目标网盘
        let dm_read = dm.read().await;
        let target = match dm_read.get_drive(&req.target_drive_id) {
            Some(d) => d,
            None => {
                let _ = app.emit("transfer-error", serde_json::json!({
                    "id": tid,
                    "error": "目标网盘未添加",
                }));
                return;
            }
        };

        // 先把数据写到临时文件，再用上传接口
        let tmp_path = format!("/tmp/litedisk_transfer_{}", tid);
        if let Err(e) = tokio::fs::write(&tmp_path, &bytes).await {
            let _ = app.emit("transfer-error", serde_json::json!({
                "id": tid,
                "error": format!("写临时文件失败: {}", e),
            }));
            return;
        }

        let remote_path = format!("{}/{}", req.target_path, req.filename);
        let d = target.read().await;
        match d.upload_file(&tmp_path, &remote_path).await {
            Ok(_) => {
                let _ = app.emit("transfer-complete", serde_json::json!({
                    "id": tid,
                    "filename": req.filename,
                }));
            }
            Err(e) => {
                let _ = app.emit("transfer-error", serde_json::json!({
                    "id": tid,
                    "error": format!("上传失败: {}", e),
                }));
            }
        }

        // 清理临时文件
        let _ = tokio::fs::remove_file(&tmp_path).await;
    });

    Ok(transfer_id)
}

// ======================== NAS 功能 ========================

#[tauri::command]
pub async fn start_nas(
    state: State<'_, AppState>,
    port: u16,
) -> Result<crate::nas::NasInfo, String> {
    let mut nas_lock = state.nas_server.write().await;
    if nas_lock.is_some() {
        return Err("NAS 已在运行".into());
    }

    let dm = state.drive_manager.clone();
    let nas = crate::nas::NasServer::new(port, dm);
    let info = nas.start().await?;
    *nas_lock = Some(nas);
    Ok(info)
}

#[tauri::command]
pub async fn stop_nas(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut nas_lock = state.nas_server.write().await;
    if let Some(nas) = nas_lock.take() {
        nas.stop().await;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_nas_status(
    state: State<'_, AppState>,
) -> Result<Option<crate::nas::NasInfo>, String> {
    let nas_lock = state.nas_server.read().await;
    Ok(nas_lock.as_ref().map(|n| n.info()))
}

// ======================== 网盘管理 ========================

#[derive(Debug, Serialize, Deserialize)]
pub struct DriveInfo {
    pub id: String,
    pub drive_type: String,
    pub logged_in: bool,
}

#[tauri::command]
pub async fn get_connected_drives(
    state: State<'_, AppState>,
) -> Result<Vec<DriveInfo>, String> {
    let dm = state.drive_manager.read().await;
    let drives = dm.get_connected_drives().await;
    Ok(drives.into_iter().map(|(id, dt, logged)| DriveInfo {
        id, drive_type: dt, logged_in: logged,
    }).collect())
}

#[derive(Debug, Deserialize)]
pub struct AddDriveRequest {
    pub drive_type: String,
    pub drive_id: String,
    pub api_key: String,
    pub secret_key: String,
    pub extra: Option<serde_json::Value>,
}

#[tauri::command]
pub async fn add_drive(
    state: State<'_, AppState>,
    req: AddDriveRequest,
) -> Result<(), String> {
    let mut dm = state.drive_manager.write().await;

    let drive: Box<dyn crate::drives::CloudDrive> = match req.drive_type.as_str() {
        "baidu" => Box::new(BaiduDrive::new(&req.drive_id, &req.api_key, &req.secret_key)),
        "aliyun" => Box::new(AliyunDrive::new(&req.drive_id, &req.api_key, &req.secret_key)),
        "webdav" => {
            let url = req.extra.as_ref()
                .and_then(|e| e["server_url"].as_str())
                .ok_or("WebDAV 需要 server_url")?;
            Box::new(WebDavDrive::new(&req.drive_id, url, &req.api_key, &req.secret_key))
        }
        "quark" => Box::new(GenericOAuthDrive::new(
            &req.drive_id, "quark", &req.api_key, &req.secret_key,
            "https://open-api.quark.cn/oauth/authorize",
            "https://open-api.quark.cn/oauth/token",
            "https://open-api.quark.cn/api/v1",
        )),
        "onetwothree" => Box::new(GenericOAuthDrive::new(
            &req.drive_id, "onetwothree", &req.api_key, &req.secret_key,
            "https://www.123pan.com/oauth/authorize",
            "https://www.123pan.com/oauth/token",
            "https://open-api.123pan.com/api/v1",
        )),
        "tianyi" => Box::new(GenericOAuthDrive::new(
            &req.drive_id, "tianyi", &req.api_key, &req.secret_key,
            "https://open.189.cn/oauth2/authorize",
            "https://open.189.cn/oauth2/token",
            "https://open.189.cn/api/v1",
        )),
        "onedrive" => Box::new(GenericOAuthDrive::new(
            &req.drive_id, "onedrive", &req.api_key, &req.secret_key,
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            "https://graph.microsoft.com/v1.0",
        )),
        "gdrive" => Box::new(GenericOAuthDrive::new(
            &req.drive_id, "gdrive", &req.api_key, &req.secret_key,
            "https://accounts.google.com/o/oauth2/v2/auth",
            "https://oauth2.googleapis.com/token",
            "https://www.googleapis.com/drive/v3",
        )),
        "pikpak" => Box::new(GenericOAuthDrive::new(
            &req.drive_id, "pikpak", &req.api_key, &req.secret_key,
            "https://user.mypikpak.com/oauth/authorize",
            "https://user.mypikpak.com/oauth/token",
            "https://api-drive.mypikpak.com/drive/v1",
        )),
        "nutstore" => {
            let url = req.extra.as_ref()
                .and_then(|e| e["server_url"].as_str())
                .unwrap_or("https://dav.jianguoyun.com/dav");
            Box::new(WebDavDrive::new(&req.drive_id, url, &req.api_key, &req.secret_key))
        }
        _ => return Err(format!("不支持的网盘类型: {}", req.drive_type)),
    };

    dm.add_drive(drive).await;
    Ok(())
}

#[tauri::command]
pub async fn remove_drive(
    state: State<'_, AppState>,
    drive_id: String,
) -> Result<(), String> {
    let mut dm = state.drive_manager.write().await;
    dm.remove_drive(&drive_id).await;
    Ok(())
}

#[tauri::command]
pub async fn get_quota(
    state: State<'_, AppState>,
    drive_id: String,
) -> Result<Quota, String> {
    let dm = state.drive_manager.read().await;
    let drive = dm.get_drive(&drive_id).ok_or("网盘未添加")?;
    let mut d = drive.write().await;
    d.get_quota().await
}
