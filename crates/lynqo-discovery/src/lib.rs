use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceInfo, ServiceEvent};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::thread;

#[derive(Debug)]
pub enum DiscoveryEvent {
    DeviceDiscovered {
        id: String,
        name: String,
        ip: String,
        port: u16,
        platform: String,
        version: String,
    },
    DeviceLost {
        id: String,
    },
}

pub struct Discovery {
    daemon: ServiceDaemon,
}

impl Discovery {
    /// Advertise the lynqo service on the local network via mDNS.
    pub fn advertise(instance_name: &str, port: u16) -> Result<Self> {
        let daemon = ServiceDaemon::new()?;

        let local_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        let hostname = format!("{}.local.", instance_name);
        let service_type = "_lynqo._tcp.local.";

        let mut props = HashMap::new();
        props.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
        props.insert("platform".to_string(), std::env::consts::OS.to_string());
        props.insert("id".to_string(), instance_name.to_string());

        let service_info = ServiceInfo::new(
            service_type,
            instance_name,
            &hostname,
            local_ip.as_str(),
            port,
            props,
        )?;

        daemon.register(service_info)?;
        tracing::info!("mDNS: advertising '{}' at {}:{}", instance_name, local_ip, port);

        Ok(Self { daemon })
    }

    /// Start browsing for other Lynqo instances on the network.
    pub fn start_browser(&self) -> Result<Receiver<DiscoveryEvent>> {
        let service_type = "_lynqo._tcp.local.";
        let browser = self.daemon.browse(service_type)?;
        let (tx, rx) = channel();

        thread::Builder::new()
            .name("lynqo-discovery-browser".into())
            .spawn(move || {
                tracing::info!("mDNS: browsing started for {}", service_type);
                while let Ok(event) = browser.recv() {
                    match event {
                        ServiceEvent::ServiceResolved(info) => {
                            let fullname = info.get_fullname();
                            let name = fullname.split('.').next().unwrap_or(fullname).to_string();
                            let ip = info.get_addresses().iter().next().map(|ip| ip.to_string()).unwrap_or_default();
                            let port = info.get_port();
                            let properties = info.get_properties();
                            let version = properties.get("version").map(|p| p.to_string()).unwrap_or_default();
                            let platform = properties.get("platform").map(|p| p.to_string()).unwrap_or_default();
                            let id = properties.get("id").map(|p| p.to_string()).unwrap_or_else(|| name.clone());

                            tracing::debug!("mDNS: resolved service '{}' at {}:{}", fullname, ip, port);
                            let _ = tx.send(DiscoveryEvent::DeviceDiscovered {
                                id,
                                name,
                                ip,
                                port,
                                platform,
                                version,
                            });
                        }
                        ServiceEvent::ServiceRemoved(_service_type, name) => {
                            tracing::debug!("mDNS: removed service '{}'", name);
                            // Fallback to name-based identifier
                            let id = name.split('.').next().unwrap_or(&name).to_string();
                            let _ = tx.send(DiscoveryEvent::DeviceLost { id });
                        }
                        _ => {}
                    }
                }
                tracing::info!("mDNS: browsing stopped");
            })?;

        Ok(rx)
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}

fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

