use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use hickory_proto::rr::Record;

#[derive(Clone)]
pub struct CacheEntry {
    pub records: Vec<Record>,
    pub expire_at: Instant,
}

#[derive(Clone)]
pub struct DnsCache {
    cache: Arc<DashMap<String, CacheEntry>>,
}

impl DnsCache {
    pub fn new() -> Self {
        let cache = Arc::new(DashMap::<String, CacheEntry>::new());
        Self { cache }
    }

    pub fn get(&self, domain: &str) -> Option<Vec<Record>> {
        if let Some(entry) = self.cache.get(domain) {
            if Instant::now() < entry.expire_at {
                return Some(entry.value().records.clone());
            } else {
                // 移除过期的缓存项
                self.cache.remove(domain);
            }
        }
        None
    }

    pub fn set(&self, domain: String, records: Vec<Record>) {
        let min_ttl = records.iter().map(|r| r.ttl()).min().unwrap_or(0);
        let expire_at = Instant::now() + Duration::from_secs(min_ttl as u64);
        let entry = CacheEntry { records, expire_at };
        self.cache.insert(domain, entry);
    }
}