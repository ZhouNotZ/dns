//request_handler.rs
use std::collections::{HashSet, HashMap};
use std::sync::Arc;
use hickory_proto::op::{Message, MessageType, ResponseCode};
use hickory_proto::rr::{Name, RecordType, RData, Record};
use hickory_resolver::TokioAsyncResolver;
use log::{error, info, warn};
use crate::resolver_manager::ResolverManager;
use crate::ip_manager::IpManager;
use crate::cache::DnsCache;
use std::net::IpAddr;

pub struct RequestHandler {
    resolver_manager: Arc<ResolverManager>,
    ip_manager: Arc<IpManager>,
    cache: Arc<DnsCache>,
    blacklist: HashSet<String>,
    custom_domain_ip: HashMap<String, IpAddr>,
    domestic_resolver: Arc<TokioAsyncResolver>,
    foreign_resolver: Arc<TokioAsyncResolver>,
}

impl RequestHandler {
    pub fn new(
        resolver_manager: Arc<ResolverManager>,
        ip_manager: Arc<IpManager>,
        cache: Arc<DnsCache>,
        blacklist: HashSet<String>,
        custom_domain_ip: HashMap<String, IpAddr>,
    ) -> Self {
        Self {
            domestic_resolver: resolver_manager.domestic_resolver.clone(),
            foreign_resolver: resolver_manager.foreign_resolver.clone(),
            resolver_manager,
            ip_manager,
            cache,
            blacklist,
            custom_domain_ip,
        }
    }

    pub async fn handle_request(&self, request: &Message) -> Message {
        let query = match request.queries().first() {
            Some(q) => q.clone(),
            None => {
                return build_response(
                    request,
                    ResponseCode::FormErr,
                    vec![],
                );
            }
        };

        let domain_name = query.name().to_utf8();
        let record_type = query.query_type();

        // 黑名单检查
        if self.blacklist.contains(&domain_name) {
            warn!("Domain '{}' is in blacklist.", domain_name);
            return build_response(request, ResponseCode::NXDomain, vec![]);
        }

        // 自定义域名IP映射
        if let Some(&ip) = self.custom_domain_ip.get(&domain_name) {
            info!("Domain '{}' matches custom IP mapping.", domain_name);
            let record = create_record(&query.name(), record_type, ip);
            return build_response(request, ResponseCode::NoError, vec![record]);
        }

        // 缓存检查
        if let Some(records) = self.cache.get(&domain_name) {
            info!("Cache hit for '{}'.", domain_name);
            return build_response(request, ResponseCode::NoError, records);
        } 

        // 特定解析器检查
        if let Some(resolver) = self.resolver_manager.get_resolver(&domain_name) {
            info!("Using specific resolver for '{}'.", domain_name);
            return self.lookup_with_resolver(request, resolver.as_ref()).await;
        }

        // 使用国内解析器解析
        info!("Using domestic resolver for '{}'.", domain_name);
        let response = self.lookup_with_resolver(request, &self.domestic_resolver).await;

        // 检查IP是否国内IP
        let ips: Vec<IpAddr> = extract_ips(&response);
        let is_domestic = ips.iter().any(|ip| self.ip_manager.is_domestic(*ip));

        if is_domestic {
            info!("Domain '{}' resolved to domestic IP.", domain_name);
            self.cache.set(domain_name, response.answers().to_vec());
            response
        } else {
            // 使用国外解析器
            info!("Domain '{}' resolved to foreign IP, retrying with foreign resolver.", domain_name);
            let response = self.lookup_with_resolver(request, &self.foreign_resolver).await;
            self.cache.set(domain_name, response.answers().to_vec());
            response
        }
    }

    async fn lookup_with_resolver(&self, request: &Message, resolver: &TokioAsyncResolver) -> Message {
        let query = request.queries().first().unwrap();
        match resolver.lookup(query.name().clone(), query.query_type()).await {
            Ok(lookup) => {
                let records = lookup.records().to_vec();
                build_response(request, ResponseCode::NoError, records)
            }
            Err(e) => {
                error!("Lookup failed for '{}': {}", query.name(), e);
                build_response(request, ResponseCode::ServFail, vec![])
            }
        }
    }
}

fn build_response(request: &Message, response_code: ResponseCode, answers: Vec<Record>) -> Message {
    let mut response = Message::new();
    response.set_id(request.id());
    response.set_message_type(MessageType::Response);
    response.set_authoritative(false);
    response.set_recursion_desired(true);
    response.set_recursion_available(true);
    response.set_response_code(response_code);
    response.add_queries(request.queries().to_vec());
    for record in answers {
        response.add_answer(record);
    }
    response
}

fn create_record(name: &Name, record_type: RecordType, ip: IpAddr) -> Record {
    let rdata = match (record_type, ip) {
        (RecordType::A, IpAddr::V4(ipv4)) => RData::A(hickory_proto::rr::rdata::A(ipv4)),
        (RecordType::AAAA, IpAddr::V6(ipv6)) => RData::AAAA(hickory_proto::rr::rdata::AAAA(ipv6)),
        _ => RData::NULL(hickory_proto::rr::rdata::NULL::with(Vec::new())),
    };
    Record::from_rdata(name.clone(), 300, rdata)
}

fn extract_ips(response: &Message) -> Vec<IpAddr> {
    response.answers().iter()
        .filter_map(|record| match record.data() {
            Some(RData::A(a_record)) => Some(IpAddr::V4(a_record.0)),
            Some(RData::AAAA(aaaa_record)) => Some(IpAddr::V6(aaaa_record.0)),
            _ => None,
        })
        .collect()
}