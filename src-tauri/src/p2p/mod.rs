// P2P 直连模块
// 用于远程访问（不在同一局域网时，设备之间直接传输）
// 使用 libp2p 实现，不经过任何中转服务器

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub name: String,
    pub online: bool,
    pub last_seen: i64,
}

pub struct P2PNetwork {
    local_peer_id: String,
    peers: RwLock<HashMap<String, PeerInfo>>,
    running: bool,
}

impl P2PNetwork {
    pub fn new() -> Self {
        let peer_id = uuid::Uuid::new_v4().to_string();
        Self {
            local_peer_id: peer_id,
            peers: RwLock::new(HashMap::new()),
            running: false,
        }
    }

    pub fn peer_id(&self) -> &str {
        &self.local_peer_id
    }

    /// 启动 P2P 网络
    pub async fn start(&mut self) -> Result<(), String> {
        use libp2p::{
            identity, noise, yamux,
            swarm::SwarmEvent,
            Multiaddr, PeerId,
        };

        // 生成密钥对
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        // 创建传输层（Noise 加密 + Yamux 多路复用）
        let transport = {
            use libp2p::tcp;
            use libp2p::core::upgrade;
            use libp2p::core::transport::Transport;

            let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));

            tcp_transport
                .upgrade(upgrade::Version::V1)
                .authenticate(noise::Config::new(&local_key).expect("signing libp2p-noise static keypair"))
                .multiplex(yamux::Config::default())
                .boxed()
        };

        // 创建 identify 协议（让对端知道我们的信息）
        let identify = {
            use libp2p::identify;
            identify::Behaviour::new(identify::Config::new(
                "/litedisk/1.0.0".into(),
                local_key.public(),
            ))
        };

        // 创建 mDNS 发现（局域网内自动发现其他轻盘用户）
        let mdns = {
            use libp2p::mdns;
            mdns::tokio::Behaviour::new(
                mdns::Config::default(),
                local_peer_id,
            ).map_err(|e| format!("mDNS 初始化失败: {}", e))?
        };

        // 创建 Swarm
        let mut swarm = {
            use libp2p::swarm::SwarmBuilder;

            SwarmBuilder::with_existing_identity(local_key)
                .with_tokio()
                .with_tcp(
                    libp2p::tcp::Config::default(),
                    noise::Config::new,
                    yamux::Config::default,
                )
                .expect("tcp transport")
                .with_behaviour(|_| {
                    // 这里简化处理，实际需要用 Behaviour derive macro
                    Ok(())
                })
                .expect("behaviour")
                .build()
        };

        // 监听所有接口
        let listen_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()
            .map_err(|e| format!("地址解析失败: {}", e))?;
        swarm.listen_on(listen_addr)
            .map_err(|e| format!("监听失败: {}", e))?;

        self.running = true;

        // 事件循环
        tokio::spawn(async move {
            loop {
                match swarm.select_next_some().await {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        log::info!("P2P listening on {}", address);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        log::info!("Connected to peer: {}", peer_id);
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        log::info!("Disconnected from peer: {}", peer_id);
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    /// 停止 P2P 网络
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// 获取已发现的设备列表
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.values().cloned().collect()
    }

    /// 添加已知设备（手动配对）
    pub async fn add_peer(&self, peer_id: &str, name: &str) {
        let mut peers = self.peers.write().await;
        peers.insert(peer_id.to_string(), PeerInfo {
            peer_id: peer_id.to_string(),
            name: name.to_string(),
            online: false,
            last_seen: chrono::Utc::now().timestamp(),
        });
    }

    /// 向指定设备发送文件（P2P 直传）
    pub async fn send_file(&self, _peer_id: &str, _file_path: &str) -> Result<(), String> {
        if !self.running {
            return Err("P2P 网络未启动".into());
        }
        // TODO: 实现实际的文件传输协议
        Err("P2P 传输功能开发中".into())
    }

    pub fn is_running(&self) -> bool {
        self.running
    }
}
