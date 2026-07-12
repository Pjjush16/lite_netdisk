// 轻盘 - 库入口（桌面端和移动端共用）

pub mod commands;
pub mod drives;
pub mod nas;
pub mod p2p;

use std::sync::Arc;
use tokio::sync::RwLock;

/// 应用全局状态
pub struct AppState {
    pub drive_manager: Arc<RwLock<drives::DriveManager>>,
    pub download_manager: Arc<RwLock<drives::DownloadManager>>,
    pub nas_server: Arc<RwLock<Option<nas::NasServer>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            drive_manager: Arc::new(RwLock::new(drives::DriveManager::new())),
            download_manager: Arc::new(RwLock::new(drives::DownloadManager::new())),
            nas_server: Arc::new(RwLock::new(None)),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            // 认证相关
            commands::get_auth_url,
            commands::exchange_token,
            commands::get_user_info,
            commands::logout,
            // 文件操作
            commands::list_files,
            commands::search_files,
            commands::get_download_link,
            commands::delete_files,
            commands::create_folder,
            commands::rename_file,
            // 下载管理
            commands::start_download,
            commands::cancel_download,
            commands::get_download_list,
            // 跨盘传输
            commands::transfer_files,
            // NAS 功能
            commands::start_nas,
            commands::stop_nas,
            commands::get_nas_status,
            // 网盘管理
            commands::get_connected_drives,
            commands::add_drive,
            commands::remove_drive,
            commands::get_quota,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
