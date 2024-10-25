//main.rs
mod config;
mod ip_manager;
mod resolver_manager;
mod cache;
mod request_handler;
mod dns_server;


use crate::config::Config;
use crate::ip_manager::IpManager;
use crate::resolver_manager::ResolverManager;
use crate::cache::DnsCache;
use crate::request_handler::RequestHandler;
use crate::dns_server::DnsServer;
use std::sync::Arc;
use env_logger::Env;
use log::info;
use std::collections::HashMap;
use std::net::IpAddr;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the IPv4 CIDR file
    #[arg(short = '6', default_value = "china_cidr_ipv4.txt", help = "Path to the IPv4 CIDR file")]
    cidr4: String,

    /// Optional path to the IPv6 CIDR file
    #[arg(short = '4',long = "cidr6", help = "Optional path to the IPv6 CIDR file")]
    cidr6: Option<String>,

    /// Path to the configuration YAML file
    #[arg(short = 'c', long, default_value = "config.yaml", help = "Path to the configuration YAML file")]
    config: String,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 8)] 
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("warn")).init();

    // 解析命令行参数
    let args = Args::parse();
    info!("DNS Server is starting...");

    // 加载配置
    let config = Config::load(&args.config);
    let config = Arc::new(config);

    // 初始化IP管理器
    let mut ip_manager = IpManager::new();

    // 加载 IPv4 CIDR 列表
    let cidr_list_ipv4 = std::fs::read_to_string(&args.cidr4)
        .unwrap_or_else(|e| {
            eprintln!("Failed to read IPv4 CIDR file '{}': {}", &args.cidr4, e);
            std::process::exit(1);
        })
        .lines()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    ip_manager.load_cidrs(&cidr_list_ipv4);

    // 如果提供了 IPv6 CIDR 文件，加载 IPv6 CIDR 列表
    if let Some(cidr6_path) = &args.cidr6 {
        let cidr_list_ipv6 = std::fs::read_to_string(cidr6_path)
            .unwrap_or_else(|e| {
                eprintln!("Failed to read IPv6 CIDR file '{}': {}", cidr6_path, e);
                std::process::exit(1);
            })
            .lines()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        ip_manager.load_cidrs(&cidr_list_ipv6);
    }

    let ip_manager = Arc::new(ip_manager);

    // 初始化解析器管理器
    let resolver_manager = ResolverManager::new(&config).await;
    let resolver_manager = Arc::new(resolver_manager);

    // 初始化缓存
    let cache = DnsCache::new();
    let cache = Arc::new(cache);

    // 初始化请求处理器
    let blacklist = config.blacklist.iter().cloned().collect();
    let custom_domain_ip = config
        .custom_domain_ip
        .iter()
        .filter_map(|(k, v)| v.parse().ok().map(|ip| (k.clone(), ip)))
        .collect::<HashMap<String, IpAddr>>();
    let request_handler = RequestHandler::new(
        resolver_manager,
        ip_manager,
        cache,
        blacklist,
        custom_domain_ip,
    );
    let request_handler = Arc::new(request_handler);

    // 启动DNS服务器
    let server_addr = format!("0.0.0.0:{}", config.server_port);
    let mut handles = Vec::new();
    for _ in 0..num_cpus::get() { // 根据 CPU 核心数量创建实例
        let request_handler = request_handler.clone();
        let server_addr = server_addr.clone();
        let handle = tokio::spawn(async move {
            let dns_server = DnsServer::new(&server_addr, request_handler).await;
            dns_server.run().await;
        });
        handles.push(handle);
    }

    
    futures::future::join_all(handles).await;
}