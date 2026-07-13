// 主应用逻辑

const App = {
    currentDriveId: null,
    currentPath: '/',
    files: [],

    // 初始化应用
    init: async function() {
        // 检查是否完成设置
        if (!Storage.isSetupDone()) {
            Router.navigate('/setup');
            return;
        }

        // 检查是否有网盘
        const drives = Storage.getDrives();
        if (drives.length === 0) {
            Router.navigate('/setup');
            return;
        }

        // 设置当前网盘
        this.currentDriveId = Storage.getCurrentDrive() || drives[0].id;

        // 注册路由
        this.registerRoutes();

        // 初始化路由
        Router.init();

        // 隐藏启动屏，显示应用
        document.getElementById('splash').style.display = 'none';
        document.getElementById('app').style.display = 'block';

        // 加载配额信息
        this.loadQuota();
    },

    // 注册路由
    registerRoutes: function() {
        Router.register('/setup', () => this.renderSetup());
        Router.register('/home', () => this.renderHome());
        Router.register('/search', () => this.renderSearch());
        Router.register('/downloads', () => this.renderDownloads());
        Router.register('/settings', () => this.renderSettings());
    },

    // ==================== 页面渲染 ====================

    renderSetup: function() {
        const main = document.getElementById('main');
        main.innerHTML = `
            <div class="container" style="max-width: 600px; margin: 40px auto;">
                <div class="card">
                    <h2>添加网盘</h2>
                    <p style="color: #666; margin: 16px 0;">选择要添加的网盘类型</p>
                    
                    <div class="form-group">
                        <label class="input-label">网盘类型</label>
                        <select id="drive-type" class="input">
                            <option value="baidu">百度网盘</option>
                            <option value="aliyun">阿里云盘</option>
                            <option value="quark">夸克网盘</option>
                            <option value="123">123云盘</option>
                            <option value="tianyi">天翼云盘</option>
                            <option value="onedrive">OneDrive</option>
                            <option value="gdrive">Google Drive</option>
                            <option value="pikpak">PikPak</option>
                            <option value="webdav">WebDAV</option>
                        </select>
                    </div>

                    <div class="form-group">
                        <label class="input-label">网盘 ID（自定义）</label>
                        <input type="text" id="drive-id" class="input" placeholder="例如：mybaidu" value="">
                    </div>

                    <div class="form-group">
                        <label class="input-label">API Key / Client ID</label>
                        <input type="text" id="api-key" class="input" placeholder="从网盘开放平台获取">
                    </div>

                    <div class="form-group">
                        <label class="input-label">Secret Key / Client Secret</label>
                        <input type="password" id="secret-key" class="input" placeholder="从网盘开放平台获取">
                    </div>

                    <div id="webdav-fields" style="display: none;">
                        <div class="form-group">
                            <label class="input-label">WebDAV 服务器地址</label>
                            <input type="text" id="webdav-url" class="input" placeholder="https://example.com/dav">
                        </div>
                    </div>

                    <button class="btn btn-primary" style="width: 100%; margin-top: 24px;" onclick="App.addDrive()">
                        添加网盘
                    </button>

                    <div style="margin-top: 24px; padding: 16px; background: #f5f5f5; border-radius: 8px; font-size: 13px; color: #666;">
                        <strong>提示：</strong>需要先到对应网盘的开放平台注册应用，获取 API Key 和 Secret Key。
                        <ul style="margin: 8px 0 0 20px; line-height: 1.8;">
                            <li>百度网盘：<a href="https://pan.baidu.com/union/apply" target="_blank">pan.baidu.com/union</a></li>
                            <li>阿里云盘：<a href="https://open.aliyundrive.com" target="_blank">open.aliyundrive.com</a></li>
                            <li>夸克网盘：<a href="https://open.quark.cn" target="_blank">open.quark.cn</a></li>
                        </ul>
                    </div>
                </div>
            </div>
        `;

        // 显示/隐藏 WebDAV 字段
        document.getElementById('drive-type').addEventListener('change', (e) => {
            document.getElementById('webdav-fields').style.display = 
                e.target.value === 'webdav' ? 'block' : 'none';
        });
    },

    addDrive: async function() {
        const type = document.getElementById('drive-type').value;
        const id = document.getElementById('drive-id').value.trim();
        const apiKey = document.getElementById('api-key').value.trim();
        const secretKey = document.getElementById('secret-key').value.trim();

        if (!id || !apiKey || !secretKey) {
            Utils.showToast('请填写完整信息', 'error');
            return;
        }

        try {
            const extra = type === 'webdav' ? {
                server_url: document.getElementById('webdav-url').value.trim()
            } : null;

            await API.addDrive({
                drive_type: type,
                drive_id: id,
                api_key: apiKey,
                secret_key: secretKey,
                extra
            });

            // 保存到本地
            Storage.addDrive({ id, type, addedAt: Date.now() });
            Storage.setSetupDone();

            Utils.showToast('网盘添加成功', 'success');
            Router.navigate('/home');
        } catch (error) {
            Utils.showToast('添加失败：' + error.message, 'error');
        }
    },

    renderHome: async function() {
        const main = document.getElementById('main');
        main.innerHTML = `
            <div class="page-header">
                <h1 class="page-title">文件</h1>
                <div class="page-actions">
                    <button class="btn btn-icon" onclick="App.refresh()" title="刷新">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path d="M23 4v6h-6M1 20v-6h6"/>
                            <path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/>
                        </svg>
                    </button>
                </div>
            </div>
            <div class="breadcrumb" id="breadcrumb"></div>
            <div id="file-list" class="file-list">
                <div class="loading">
                    <div class="spinner"></div>
                    <div class="loading-text">加载中...</div>
                </div>
            </div>
        `;

        await this.loadFiles(this.currentPath);
    },

    renderSearch: function() {
        const main = document.getElementById('main');
        main.innerHTML = `
            <div class="page-header">
                <h1 class="page-title">搜索</h1>
            </div>
            <div style="padding: 20px;">
                <div class="search-box">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <circle cx="11" cy="11" r="8"/>
                        <line x1="21" y1="21" x2="16.65" y2="16.65"/>
                    </svg>
                    <input type="text" id="search-input" class="input" placeholder="搜索文件...">
                </div>
                <div id="search-results" style="margin-top: 20px;"></div>
            </div>
        `;

        document.getElementById('search-input').addEventListener('input', 
            Utils.debounce((e) => this.search(e.target.value), 500)
        );
    },

    renderDownloads: async function() {
        const main = document.getElementById('main');
        main.innerHTML = `
            <div class="page-header">
                <h1 class="page-title">下载</h1>
            </div>
            <div style="padding: 20px;">
                <div id="download-list">
                    <div class="loading">
                        <div class="spinner"></div>
                        <div class="loading-text">加载中...</div>
                    </div>
                </div>
            </div>
        `;

        await this.loadDownloads();
    },

    renderSettings: function() {
        const drives = Storage.getDrives();
        const drivesHtml = drives.map(d => `
            <div class="settings-item">
                <div style="display: flex; justify-content: space-between; align-items: center;">
                    <div>
                        <div class="settings-item-label">${d.id} (${d.type})</div>
                        <div class="settings-item-desc">添加于 ${new Date(d.addedAt).toLocaleDateString()}</div>
                    </div>
                    <button class="btn btn-text" onclick="App.removeDrive('${d.id}')">删除</button>
                </div>
            </div>
        `).join('');

        const main = document.getElementById('main');
        main.innerHTML = `
            <div class="page-header">
                <h1 class="page-title">设置</h1>
            </div>
            <div style="padding: 20px;">
                <div class="card">
                    <h3 class="card-title">网盘管理</h3>
                    ${drivesHtml || '<div class="settings-item-desc">暂无网盘</div>'}
                    <button class="btn btn-primary" style="margin-top: 16px;" onclick="Router.navigate('/setup')">
                        添加网盘
                    </button>
                </div>

                <div class="card" style="margin-top: 16px;">
                    <h3 class="card-title">关于</h3>
                    <div class="settings-item">
                        <div class="settings-item-label">轻盘 LiteDisk</div>
                        <div class="settings-item-desc">版本 1.0.0</div>
                    </div>
                </div>
            </div>
        `;
    },

    // ==================== 文件操作 ====================

    loadFiles: async function(path) {
        this.currentPath = path;
        this.updateBreadcrumb();

        const fileList = document.getElementById('file-list');
        fileList.innerHTML = '<div class="loading"><div class="spinner"></div><div class="loading-text">加载中...</div></div>';

        try {
            this.files = await API.listFiles(this.currentDriveId, path);
            this.renderFileList();
        } catch (error) {
            fileList.innerHTML = `<div class="empty-state"><p>加载失败：${error.message}</p></div>`;
        }
    },

    renderFileList: function() {
        const fileList = document.getElementById('file-list');
        
        if (this.files.length === 0) {
            fileList.innerHTML = '<div class="empty-state"><p>空文件夹</p></div>';
            return;
        }

        const html = this.files.map(file => `
            <div class="file-item" onclick="App.clickFile('${file.id}', ${file.is_dir})">
                <div class="file-icon ${Utils.getFileIconClass(file.category)}">
                    ${file.thumbnail 
                        ? `<img src="${file.thumbnail}" alt="">` 
                        : Utils.getFileIcon(file.category)
                    }
                </div>
                <div class="file-info">
                    <div class="file-name">${file.name}</div>
                    <div class="file-meta">
                        ${file.is_dir ? '' : Utils.formatSize(file.size)}
                        ${file.modified ? ' · ' + Utils.formatDate(file.modified) : ''}
                    </div>
                </div>
                <div class="file-actions">
                    ${!file.is_dir ? `
                        <button class="btn btn-icon" onclick="event.stopPropagation(); App.download('${file.id}', '${file.name}')" title="下载">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"/>
                            </svg>
                        </button>
                    ` : ''}
                    <button class="btn btn-icon" onclick="event.stopPropagation(); App.deleteFile('${file.path}')" title="删除">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/>
                        </svg>
                    </button>
                </div>
            </div>
        `).join('');

        fileList.innerHTML = html;
    },

    clickFile: function(fileId, isDir) {
        if (isDir) {
            const file = this.files.find(f => f.id === fileId);
            if (file) {
                this.loadFiles(file.path);
            }
        } else {
            // 可以打开预览或下载
            this.download(fileId, this.files.find(f => f.id === fileId)?.name || 'file');
        }
    },

    download: async function(fileId, filename) {
        try {
            const savePath = Storage.getDownloadPath() + '/' + filename;
            await API.startDownload({
                drive_id: this.currentDriveId,
                file_id: fileId,
                filename: filename,
                save_path: savePath
            });
            Utils.showToast('开始下载', 'success');
        } catch (error) {
            Utils.showToast('下载失败：' + error.message, 'error');
        }
    },

    deleteFile: function(path) {
        Utils.confirm('确认删除', '确定要删除这个文件吗？', async () => {
            try {
                await API.deleteFiles(this.currentDriveId, [path]);
                Utils.showToast('删除成功', 'success');
                this.loadFiles(this.currentPath);
            } catch (error) {
                Utils.showToast('删除失败：' + error.message, 'error');
            }
        });
    },

    refresh: function() {
        this.loadFiles(this.currentPath);
    },

    updateBreadcrumb: function() {
        const breadcrumb = document.getElementById('breadcrumb');
        const parts = this.currentPath.split('/').filter(p => p);
        
        let html = `<a href="#" onclick="App.loadFiles('/')" class="breadcrumb-item">根目录</a>`;
        let path = '';
        
        parts.forEach((part, i) => {
            path += '/' + part;
            const isLast = i === parts.length - 1;
            html += `<span class="breadcrumb-sep">/</span>`;
            html += isLast
                ? `<span class="breadcrumb-item current">${part}</span>`
                : `<a href="#" onclick="App.loadFiles('${path}')" class="breadcrumb-item">${part}</a>`;
        });
        
        breadcrumb.innerHTML = html;
    },

    // ==================== 搜索 ====================

    search: async function(keyword) {
        const results = document.getElementById('search-results');
        
        if (!keyword.trim()) {
            results.innerHTML = '';
            return;
        }

        results.innerHTML = '<div class="loading"><div class="spinner"></div></div>';

        try {
            const files = await API.searchFiles(this.currentDriveId, keyword);
            
            if (files.length === 0) {
                results.innerHTML = '<div class="empty-state"><p>未找到文件</p></div>';
                return;
            }

            const html = files.map(file => `
                <div class="file-item" onclick="App.clickSearchResult('${file.id}', '${file.path}', ${file.is_dir})">
                    <div class="file-icon ${Utils.getFileIconClass(file.category)}">
                        ${Utils.getFileIcon(file.category)}
                    </div>
                    <div class="file-info">
                        <div class="file-name">${file.name}</div>
                        <div class="file-meta">${file.is_dir ? '文件夹' : Utils.formatSize(file.size)}</div>
                    </div>
                </div>
            `).join('');

            results.innerHTML = html;
        } catch (error) {
            results.innerHTML = `<div class="empty-state"><p>搜索失败：${error.message}</p></div>`;
        }
    },

    clickSearchResult: function(fileId, path, isDir) {
        if (isDir) {
            Router.navigate('/home');
            setTimeout(() => this.loadFiles(path), 100);
        } else {
            Router.navigate('/home');
            setTimeout(() => this.download(fileId, this.files.find(f => f.id === fileId)?.name || 'file'), 100);
        }
    },

    // ==================== 下载管理 ====================

    loadDownloads: async function() {
        const list = document.getElementById('download-list');
        
        try {
            const tasks = await API.getDownloadList();
            
            if (tasks.length === 0) {
                list.innerHTML = '<div class="empty-state"><p>暂无下载任务</p></div>';
                return;
            }

            const html = tasks.map(task => `
                <div class="card" style="margin-bottom: 12px;">
                    <div class="card-title" style="margin-bottom: 8px;">${task.filename}</div>
                    <div class="card-text" style="font-size: 13px; color: #666;">
                        ${task.status} - ${Utils.formatSize(task.downloaded)} / ${Utils.formatSize(task.total_size)}
                    </div>
                    <div class="progress" style="margin-top: 12px;">
                        <div class="progress-bar" style="width: ${task.total_size > 0 ? (task.downloaded / task.total_size * 100) : 0}%"></div>
                    </div>
                    ${task.status === 'downloading' ? `
                        <button class="btn btn-text" style="margin-top: 8px;" onclick="App.cancelDownload('${task.id}')">
                            取消
                        </button>
                    ` : ''}
                </div>
            `).join('');

            list.innerHTML = html;
        } catch (error) {
            list.innerHTML = `<div class="empty-state"><p>加载失败：${error.message}</p></div>`;
        }
    },

    cancelDownload: async function(taskId) {
        try {
            await API.cancelDownload(taskId);
            Utils.showToast('已取消', 'success');
            this.loadDownloads();
        } catch (error) {
            Utils.showToast('取消失败：' + error.message, 'error');
        }
    },

    // ==================== 网盘管理 ====================

    removeDrive: function(driveId) {
        Utils.confirm('确认删除', '确定要删除这个网盘吗？', async () => {
            try {
                await API.removeDrive(driveId);
                Storage.removeDrive(driveId);
                Utils.showToast('删除成功', 'success');
                this.renderSettings();
            } catch (error) {
                Utils.showToast('删除失败：' + error.message, 'error');
            }
        });
    },

    // ==================== 配额 ====================

    loadQuota: async function() {
        try {
            const quota = await API.getQuota(this.currentDriveId);
            const used = Utils.formatSize(quota.used);
            const total = Utils.formatSize(quota.total);
            const percent = (quota.used / quota.total * 100).toFixed(1);
            
            document.getElementById('quota-fill').style.width = percent + '%';
            document.getElementById('quota-text').textContent = `${used} / ${total} (${percent}%)`;
        } catch (error) {
            console.error('加载配额失败:', error);
        }
    }
};

// 初始化应用
window.addEventListener('DOMContentLoaded', () => App.init());
