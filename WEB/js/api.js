// API 调用封装（Tauri invoke）

const API = {
    // 检查是否在 Tauri 环境中
    isTauri: function() {
        return window.__TAURI__ !== undefined;
    },

    // 调用 Tauri 命令
    invoke: async function(cmd, args = {}) {
        if (!this.isTauri()) {
            console.warn('Not in Tauri environment');
            throw new Error('不在应用环境中');
        }
        try {
            return await window.__TAURI__.invoke(cmd, args);
        } catch (error) {
            console.error(`API call failed: ${cmd}`, error);
            throw error;
        }
    },

    // ==================== 认证相关 ====================
    
    getAuthUrl: async function(driveId) {
        return await this.invoke('get_auth_url', { driveId });
    },

    exchangeToken: async function(driveId, code) {
        return await this.invoke('exchange_token', { driveId, code });
    },

    getUserInfo: async function(driveId) {
        return await this.invoke('get_user_info', { driveId });
    },

    logout: async function(driveId) {
        return await this.invoke('logout', { driveId });
    },

    // ==================== 文件操作 ====================

    listFiles: async function(driveId, path) {
        return await this.invoke('list_files', { driveId, path });
    },

    searchFiles: async function(driveId, keyword) {
        return await this.invoke('search_files', { driveId, keyword });
    },

    getDownloadLink: async function(driveId, fileId) {
        return await this.invoke('get_download_link', { driveId, fileId });
    },

    deleteFiles: async function(driveId, paths) {
        return await this.invoke('delete_files', { driveId, paths });
    },

    createFolder: async function(driveId, path) {
        return await this.invoke('create_folder', { driveId, path });
    },

    renameFile: async function(driveId, path, newName) {
        return await this.invoke('rename_file', { driveId, path, newName });
    },

    // ==================== 下载管理 ====================

    startDownload: async function(req) {
        return await this.invoke('start_download', { req });
    },

    cancelDownload: async function(taskId) {
        return await this.invoke('cancel_download', { taskId });
    },

    getDownloadList: async function() {
        return await this.invoke('get_download_list');
    },

    // ==================== 跨盘传输 ====================

    transferFiles: async function(req) {
        return await this.invoke('transfer_files', { req });
    },

    // ==================== NAS 功能 ====================

    startNas: async function(port) {
        return await this.invoke('start_nas', { port });
    },

    stopNas: async function() {
        return await this.invoke('stop_nas');
    },

    getNasStatus: async function() {
        return await this.invoke('get_nas_status');
    },

    // ==================== 网盘管理 ====================

    getConnectedDrives: async function() {
        return await this.invoke('get_connected_drives');
    },

    addDrive: async function(req) {
        return await this.invoke('add_drive', { req });
    },

    removeDrive: async function(driveId) {
        return await this.invoke('remove_drive', { driveId });
    },

    getQuota: async function(driveId) {
        return await this.invoke('get_quota', { driveId });
    },

    // ==================== 事件监听 ====================

    onDownloadProgress: function(callback) {
        if (this.isTauri()) {
            window.__TAURI__.event.listen('download-progress', callback);
        }
    },

    onTransferProgress: function(callback) {
        if (this.isTauri()) {
            window.__TAURI__.event.listen('transfer-progress', callback);
        }
    },

    onTransferComplete: function(callback) {
        if (this.isTauri()) {
            window.__TAURI__.event.listen('transfer-complete', callback);
        }
    },

    onTransferError: function(callback) {
        if (this.isTauri()) {
            window.__TAURI__.event.listen('transfer-error', callback);
        }
    }
};

