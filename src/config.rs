//config.rs
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server_port: u16,
    pub domestic_dns: Vec<String>,
    pub foreign_dns: Vec<String>,
    pub blacklist: Vec<String>,
    pub domain_specific_dns: HashMap<String, Vec<String>>,
    pub custom_domain_ip: HashMap<String, String>,
}

impl Config {
    pub fn load(config_file: &str) -> Self {
        let contents = std::fs::read_to_string(config_file).unwrap_or_else(|e| {
            eprintln!("Failed to read configuration file '{}': {}", config_file, e);
            std::process::exit(1);
        });

        let mut config: Config = serde_yaml::from_str(&contents).unwrap_or_else(|e| {
            eprintln!("Failed to parse configuration file '{}': {}", config_file, e);
            std::process::exit(1);
        });

        // 确保域名以 '.' 结尾
        config.blacklist = config
            .blacklist
            .into_iter()
            .map(|domain| Self::ensure_trailing_dot(&domain))
            .collect();

        for (_, domains) in config.domain_specific_dns.iter_mut() {
            *domains = domains
                .into_iter()
                .map(|domain| Self::ensure_trailing_dot(&domain))
                .collect();
        }

        config.custom_domain_ip = config
            .custom_domain_ip
            .into_iter()
            .map(|(domain, ip)| (Self::ensure_trailing_dot(&domain), ip))
            .collect();

        config
    }

    fn ensure_trailing_dot(domain: &str) -> String {
        if domain.ends_with('.') {
            domain.to_string()
        } else {
            format!("{}.", domain)
        }
    }
}