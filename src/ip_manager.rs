//ip_manager.rs
use std::net::IpAddr;
use ipnet::IpNet;
use std::collections::HashSet;

pub struct IpManager {
    cidr_set: HashSet<IpNet>,
}

impl IpManager {
    pub fn new() -> Self {
        Self {
            cidr_set: HashSet::new(),
        }
    }

    pub fn load_cidrs(&mut self, cidr_list: &[String]) {
        for cidr in cidr_list {
            if let Ok(ipnet) = cidr.parse::<IpNet>() {
                self.cidr_set.insert(ipnet);
            }
        }
    }

    pub fn is_domestic(&self, ip: IpAddr) -> bool {
        self.cidr_set.iter().any(|cidr| cidr.contains(&ip))
    }
}