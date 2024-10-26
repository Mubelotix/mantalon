use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::RwLock;
use log::*;

pub type DnsCache = Arc<RwLock<HashMap<String, (u64, Vec<IpAddr>)>>>;

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

#[cfg(feature = "custom_dns")]
pub async fn resolve(cache: DnsCache, domain: &str, dns_provider: SocketAddr) -> Vec<IpAddr> {
    use trust_dns_client::client::{AsyncClient, ClientHandle};
    use trust_dns_client::rr::{DNSClass, Name, RData, RecordType};
    use trust_dns_client::tcp::TcpClientStream;
    use trust_dns_client::proto::iocompat::AsyncIoTokioAsStd;
    use futures::future::join_all;
    use tokio::net::TcpStream;
    use std::str::FromStr;

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

#[cfg(not(feature = "custom_dns"))]
pub async fn resolve(cache: DnsCache, domain: &str, _dns_provider: SocketAddr) -> Vec<IpAddr> {
    use std::{net::{SocketAddr, ToSocketAddrs}, thread, io::Result as IoResult, vec::IntoIter};
    use tokio::sync::oneshot;

    // Check cache
    if let Some((ttl, ips)) = cache.read().await.get(domain) {
        if now() <= *ttl && !ips.is_empty() {
            return ips.clone();
        }
    }

    // Resolve domain in another thread
    let (sender, receiver) = oneshot::channel::<IoResult<IntoIter<SocketAddr>>>();
    let socket_addr = format!("{domain}:0");
    thread::spawn(move || {
        let result = socket_addr.to_socket_addrs();
        let _ = sender.send(result);
    });

    // Wait and process the results
    let result = receiver.await.expect("The resolver thread should not drop");
    match result {
        Ok(addrs) => {
            let ips = addrs.map(|addr| addr.ip()).collect::<Vec<IpAddr>>();
            let hypothetical_ttl = now() + 5*60;
            cache.write().await.insert(domain.to_owned(), (hypothetical_ttl, ips.clone()));
            ips
        }
        Err(e) => {
            error!("Failed to resolve domain {domain}: {e}");
            Vec::new()
        }
    }
}
