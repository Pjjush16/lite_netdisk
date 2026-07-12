// 本地存储服务

const Storage = {
    keys: {
        SETUP_DONE: 'litedisk_setup_done',
        DRIVES: 'litedisk_drives',
        CURRENT_DRIVE: 'litedisk_current_drive',
        THEME: 'litedisk_theme',
        DOWNLOAD_PATH: 'litedisk_download_path'
    },

    // 检查是否完成初始设置
    isSetupDone: function() {
        return localStorage.getItem(this.keys.SETUP_DONE) === 'true';
    },

    // 标记设置完成
    setSetupDone: function() {
        localStorage.setItem(this.keys.SETUP_DONE, 'true');
    },

    // 获取已添加的网盘列表
    getDrives: function() {
        const data = localStorage.getItem(this.keys.DRIVES);
        return data ? JSON.parse(data) : [];
    },

    // 保存网盘列表
    setDrives: function(drives) {
        localStorage.setItem(this.keys.DRIVES, JSON.stringify(drives));
    },

    // 添加网盘
    addDrive: function(drive) {
        const drives = this.getDrives();
        drives.push(drive);
        this.setDrives(drives);
    },

    // 删除网盘
    removeDrive: function(driveId) {
        const drives = this.getDrives().filter(d => d.id !== driveId);
        this.setDrives(drives);
    },

    // 获取当前选中的网盘
    getCurrentDrive: function() {
        return localStorage.getItem(this.keys.CURRENT_DRIVE);
    },

    // 设置当前网盘
    setCurrentDrive: function(driveId) {
        localStorage.setItem(this.keys.CURRENT_DRIVE, driveId);
    },

    // 获取主题
    getTheme: function() {
        return localStorage.getItem(this.keys.THEME) || 'auto';
    },

    // 设置主题
    setTheme: function(theme) {
        localStorage.setItem(this.keys.THEME, theme);
    },

    // 获取下载路径
    getDownloadPath: function() {
        return localStorage.getItem(this.keys.DOWNLOAD_PATH) || '~/Downloads';
    },

    // 设置下载路径
    setDownloadPath: function(path) {
        localStorage.setItem(this.keys.DOWNLOAD_PATH, path);
    },

    // 清除所有数据
    clear: function() {
        Object.values(this.keys).forEach(key => {
            localStorage.removeItem(key);
        });
    }
};
