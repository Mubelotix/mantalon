use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use futures::future::join_all;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use trust_dns_client::client::{AsyncClient, ClientHandle};
use trust_dns_client::rr::{DNSClass, Name, RData, RecordType};
use trust_dns_client::tcp::TcpClientStream;
use trust_dns_client::proto::iocompat::AsyncIoTokioAsStd;
use log::*;

pub type DnsCache = Arc<RwLock<HashMap<String, (u64, Vec<IpAddr>)>>>;

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

pub async fn resolve(cache: DnsCache, domain: &str, dns_provider: SocketAddr) -> Vec<IpAddr> {
    // Check cache
    if let Some((ttl, ips)) = cache.read().await.get(domain) {
        if now() <= *ttl && !ips.is_empty() {
            return ips.clone();
        }
    }

    // Create client
    let (stream, sender) = TcpClientStream::<AsyncIoTokioAsStd<TcpStream>>::new(dns_provider);
    let client = AsyncClient::new(stream, sender, None);
    let Ok((mut client, bg)) = client.await else {
        error!("Failed to connect to DNS provider");
        return Vec::new();
    };
    tokio::spawn(bg);

    // Build queries
    let mut queries = Vec::new();
    let Ok(name) = Name::from_str(domain) else {
        error!("Invalid domain name: {domain}");
        return Vec::new();
    };
    let aaaa_query = client.query(name.clone(), DNSClass::IN, RecordType::AAAA);
    queries.push(aaaa_query);
    let a_query = client.query(name, DNSClass::IN, RecordType::A);
    queries.push(a_query);

    // Read results
    let results = join_all(queries).await;
    let mut ips = Vec::new();
    for resp in results.into_iter().filter_map(|res| res.ok()) {
        for answer in resp.answers() {
            match answer.data() {
                Some(RData::A(ip)) => ips.push(IpAddr::V4(ip.0)),
                Some(RData::AAAA(ip)) => ips.push(IpAddr::V6(ip.0)),
                _ => {}
            }
        }
    }

    // Cache results
    let ttl = now() + 5*60;
    cache.write().await.insert(domain.to_owned(), (ttl, ips.clone()));

    ips
}
