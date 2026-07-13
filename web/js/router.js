// 路由管理

const Router = {
    current: null,
    routes: {},

    // 注册路由
    register: function(path, handler) {
        this.routes[path] = handler;
    },

    // 初始化路由
    init: function() {
        window.addEventListener('hashchange', () => this.handleRoute());
        this.handleRoute();
    },

    // 处理路由
    handleRoute: function() {
        const hash = window.location.hash.slice(1) || '/';
        const [path, ...params] = hash.split('/');
        
        const route = '/' + path;
        
        if (this.routes[route]) {
            this.current = route;
            this.routes[route](params);
            this.updateNav(route);
        } else {
            // 默认首页
            window.location.hash = '#/home';
        }
    },

    // 更新导航栏高亮
    updateNav: function(route) {
        document.querySelectorAll('.nav-item, .bottom-nav-item').forEach(item => {
            item.classList.remove('active');
            if (item.dataset.page === route.slice(1)) {
                item.classList.add('active');
            }
        });
    },

    // 导航到指定页面
    navigate: function(path) {
        window.location.hash = '#' + path;
    }
};
