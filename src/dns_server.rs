//dns_server.rs
use tokio::net::UdpSocket;
use tokio::sync::Semaphore;
use std::sync::Arc;
use hickory_proto::op::Message;
use log::{error, info};
use crate::request_handler::RequestHandler;
use std::os::unix::io::AsRawFd;
use nix::sys::socket::{setsockopt, sockopt::ReusePort};
pub struct DnsServer {
    socket: Arc<UdpSocket>,
    request_handler: Arc<RequestHandler>,
    semaphore: Arc<Semaphore>,
}

impl DnsServer {
    pub async fn new(addr: &str, request_handler: Arc<RequestHandler>) -> Self {
        // 使用标准库的 UdpSocket
        let std_socket = std::net::UdpSocket::bind(addr).expect("Failed to bind UDP socket");
        // 设置 SO_REUSEPORT 选项
        setsockopt(std_socket.as_raw_fd(), ReusePort, &true).expect("Failed to set SO_REUSEPORT");
        // 将标准库的 UdpSocket 转换为 Tokio 的 UdpSocket
        let socket = UdpSocket::from_std(std_socket).expect("Failed to create Tokio UdpSocket");
    
        info!("DNS Server is running on {}", addr);
        Self {
            socket: Arc::new(socket),
            request_handler,
            semaphore: Arc::new(Semaphore::new(5000)), // 控制并发数量
        }
    }

    pub async fn run(&self) {
        let mut buf = [0u8; 512];
        loop {
            let (len, src) = match self.socket.recv_from(&mut buf).await {
                Ok((len, src)) => (len, src),
                Err(e) => {
                    error!("Failed to receive UDP packet: {}", e);
                    continue;
                }
            };

            let request_data = buf[..len].to_vec();
            let handler = self.request_handler.clone();
            let socket = self.socket.clone();
            let permit = self.semaphore.clone().acquire_owned().await.unwrap();

            tokio::spawn(async move {
                let _permit = permit; // 保持permit的生命周期
                let request = match Message::from_vec(&request_data) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!("Failed to parse DNS request: {}", e);
                        return;
                    }
                };

                let response = handler.handle_request(&request).await;
                let response_data = match response.to_vec() {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to serialize DNS response: {}", e);
                        return;
                    }
                };

                if let Err(e) = socket.send_to(&response_data, src).await {
                    error!("Failed to send response: {}", e);
                }
            });
        }
    }
}