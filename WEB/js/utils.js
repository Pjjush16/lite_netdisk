// 工具函数

const Utils = {
    // 格式化文件大小
    formatSize: function(bytes) {
        if (bytes === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return (bytes / Math.pow(k, i)).toFixed(2) + ' ' + sizes[i];
    },

    // 格式化时间
    formatDate: function(timestamp) {
        const date = new Date(timestamp * 1000);
        const year = date.getFullYear();
        const month = String(date.getMonth() + 1).padStart(2, '0');
        const day = String(date.getDate()).padStart(2, '0');
        const hour = String(date.getHours()).padStart(2, '0');
        const min = String(date.getMinutes()).padStart(2, '0');
        return `${year}-${month}-${day} ${hour}:${min}`;
    },

    // 显示 Toast
    showToast: function(message, type = 'info') {
        const container = document.getElementById('toast-container');
        const toast = document.createElement('div');
        toast.className = `toast toast-${type}`;
        toast.textContent = message;
        container.appendChild(toast);
        setTimeout(() => toast.remove(), 3000);
    },

    // 显示模态框
    showModal: function(title, content, buttons = []) {
        const overlay = document.getElementById('modal-overlay');
        const container = document.getElementById('modal-container');
        
        const modal = document.createElement('div');
        modal.className = 'modal';
        modal.innerHTML = `
            <div class="modal-title">${title}</div>
            <div class="modal-body">${content}</div>
            <div class="modal-footer" id="modal-footer"></div>
        `;
        
        const footer = modal.querySelector('#modal-footer');
        buttons.forEach(btn => {
            const button = document.createElement('button');
            button.className = `btn ${btn.className || 'btn-secondary'}`;
            button.textContent = btn.text;
            button.onclick = () => {
                if (btn.action) btn.action();
                closeModal();
            };
            footer.appendChild(button);
        });
        
        container.innerHTML = '';
        container.appendChild(modal);
        overlay.style.display = 'block';
        container.style.display = 'block';
        
        overlay.onclick = closeModal;
        
        function closeModal() {
            overlay.style.display = 'none';
            container.style.display = 'none';
        }
        
        return closeModal;
    },

    // 确认对话框
    confirm: function(title, message, onConfirm) {
        this.showModal(title, message, [
            { text: '取消', className: 'btn-secondary' },
            { text: '确定', className: 'btn-primary', action: onConfirm }
        ]);
    },

    // 获取文件图标类名
    getFileIconClass: function(category) {
        const icons = {
            'folder': 'folder',
            'video': 'video',
            'audio': 'audio',
            'image': 'image',
            'document': 'doc',
            'app': 'app',
            'other': 'other'
        };
        return icons[category] || 'other';
    },

    // 获取文件图标 SVG
    getFileIcon: function(category) {
        const icons = {
            folder: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/></svg>',
            video: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M17 10.5V7c0-.55-.45-1-1-1H4c-.55 0-1 .45-1 1v10c0 .55.45 1 1 1h12c.55 0 1-.45 1-1v-3.5l4 4v-11l-4 4z"/></svg>',
            audio: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/></svg>',
            image: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M21 19V5c0-1.1-.9-2-2-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2zM8.5 13.5l2.5 3.01L14.5 12l4.5 6H5l3.5-4.5z"/></svg>',
            doc: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M14 2H6c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm2 16H8v-2h8v2zm0-4H8v-2h8v2zm-3-5V3.5L18.5 9H13z"/></svg>',
            app: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M17 1.01L7 1c-1.1 0-2 .9-2 2v18c0 1.1.9 2 2 2h10c1.1 0 2-.9 2-2V3c0-1.1-.9-1.99-2-1.99zM17 19H7V5h10v14z"/></svg>',
            other: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M6 2c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6H6zm7 7V3.5L18.5 9H13z"/></svg>'
        };
        return icons[category] || icons.other;
    },

    // URL 编码
    urlEncode: function(str) {
        return encodeURIComponent(str);
    },

    // 防抖
    debounce: function(func, wait) {
        let timeout;
        return function(...args) {
            clearTimeout(timeout);
            timeout = setTimeout(() => func.apply(this, args), wait);
        };
    },

    // 节流
    throttle: function(func, limit) {
        let inThrottle;
        return function(...args) {
            if (!inThrottle) {
                func.apply(this, args);
                inThrottle = true;
                setTimeout(() => inThrottle = false, limit);
            }
        };
    }
};
