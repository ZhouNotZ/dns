//resolver_manager.rs
use std::collections::HashMap;
use std::sync::Arc;
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfigGroup, NameServerConfig, Protocol};
use crate::config::Config;
use std::net::ToSocketAddrs;

pub struct ResolverManager {
    pub domestic_resolver: Arc<TokioAsyncResolver>,
    pub foreign_resolver: Arc<TokioAsyncResolver>,
    domain_specific_resolvers: HashMap<String, Arc<TokioAsyncResolver>>,
    wildcard_resolvers: Vec<(String, Arc<TokioAsyncResolver>)>,
}

impl ResolverManager {
    pub async fn new(config: &Config) -> Self {
        let domestic_resolver = Self::create_resolver(&config.domestic_dns).await;
        let foreign_resolver = Self::create_resolver(&config.foreign_dns).await;

        let mut domain_specific_resolvers = HashMap::new();
        let mut wildcard_resolvers = Vec::new();

        for (server_addr, domains) in &config.domain_specific_dns {
            let resolver = Self::create_resolver(&vec![server_addr.clone()]).await;
            let resolver = Arc::new(resolver); // 将 resolver 包装在 Arc 中

            for domain in domains {
                if let Some(suffix) = domain.strip_prefix("*.") {
                    wildcard_resolvers.push((suffix.to_string(), resolver.clone()));
                } else {
                    domain_specific_resolvers.insert(domain.clone(), resolver.clone());
                }
            }
        }

        Self {
            domestic_resolver: Arc::new(domestic_resolver),
            foreign_resolver: Arc::new(foreign_resolver),
            domain_specific_resolvers,
            wildcard_resolvers,
        }
    }

    async fn create_resolver(servers: &[String]) -> TokioAsyncResolver {
        let name_servers = servers.iter()
            .filter_map(|s| {
                let socket_addr = (s.as_str(), 53)
                    .to_socket_addrs()
                    .ok()?
                    .next()?;
                Some(NameServerConfig {
                    socket_addr,
                    protocol: Protocol::Udp,
                    tls_dns_name: None,
                    trust_negative_responses: true,
                    bind_addr: None,
                })
            })
            .collect::<Vec<NameServerConfig>>();

        TokioAsyncResolver::tokio(
            ResolverConfig::from_parts(
                None,
                vec![],
                NameServerConfigGroup::from(name_servers),
            ),
            ResolverOpts::default(),
        )
    }

    pub fn get_resolver(&self, domain: &str) -> Option<Arc<TokioAsyncResolver>> {
        if let Some(resolver) = self.domain_specific_resolvers.get(domain) {
            return Some(resolver.clone());
        }

        for (suffix, resolver) in &self.wildcard_resolvers {
            if domain.ends_with(suffix) {
                return Some(resolver.clone());
            }
        }

        None
    }
}