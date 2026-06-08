use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};

pub const SERVICE_TYPE: &str = "_hopper._tcp.local.";

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub addresses: Vec<IpAddr>,
}

pub struct Discovery {
    pub daemon: ServiceDaemon,
    fullname: Option<String>,
}

impl Discovery {
    pub fn new() -> Result<Self> {
        Ok(Self {
            daemon: ServiceDaemon::new()?,
            fullname: None,
        })
    }

    pub fn advertise(&mut self, instance_name: &str, port: u16) -> Result<()> {
        let ip = local_ipv4();
        let hostname = system_hostname();
        let host_name = format!("{}.local.", hostname);

        let info = ServiceInfo::new(
            SERVICE_TYPE,
            instance_name,
            &host_name,
            ip.to_string().as_str(),
            port,
            None::<HashMap<String, String>>,
        )?;

        self.fullname = Some(info.get_fullname().to_string());
        self.daemon.register(info)?;
        Ok(())
    }

    pub fn stop_advertise(&mut self) -> Result<()> {
        if let Some(ref name) = self.fullname {
            self.daemon.unregister(name)?;
            self.fullname = None;
        }
        Ok(())
    }

    /// Returns a channel that yields PeerInfo as peers are discovered.
    pub fn browse(&self) -> Result<std::sync::mpsc::Receiver<PeerInfo>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let receiver = self.daemon.browse(SERVICE_TYPE)?;

        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                if let ServiceEvent::ServiceResolved(info) = event {
                    let peer = PeerInfo {
                        name: info.get_fullname().to_string(),
                        host: info.get_hostname().to_string(),
                        port: info.get_port(),
                        addresses: info
                            .get_addresses()
                            .iter()
                            .map(|a| *a)
                            .collect(),
                    };
                    if tx.send(peer).is_err() {
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }

    pub fn shutdown(self) -> Result<()> {
        self.daemon.shutdown()?;
        Ok(())
    }
}

fn local_ipv4() -> Ipv4Addr {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").unwrap_or_else(|_| unreachable!());
    // Connecting a UDP socket doesn't send any packets; it just sets the route.
    let _ = sock.connect("8.8.8.8:80");
    match sock.local_addr().map(|a| a.ip()) {
        Ok(IpAddr::V4(ip)) => ip,
        _ => Ipv4Addr::new(127, 0, 0, 1),
    }
}

fn system_hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "hopper-node".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_starts_and_stops() {
        let d = Discovery::new().expect("daemon new");
        d.shutdown().expect("shutdown");
    }

    #[test]
    fn advertise_and_browse_local() {
        let mut d = Discovery::new().expect("daemon");
        d.advertise("hopper-test", 15900).expect("advertise");
        let rx = d.browse().expect("browse");

        // Give mDNS time to resolve on loopback
        std::thread::sleep(std::time::Duration::from_millis(1500));

        let mut found = false;
        while let Ok(peer) = rx.try_recv() {
            if peer.name.contains("hopper-test") {
                found = true;
            }
        }

        d.stop_advertise().ok();
        d.shutdown().ok();
        assert!(found, "did not discover own mDNS advertisement");
    }
}
