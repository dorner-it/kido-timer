//! LAN addresses of the laptop, judge URL and QR code.

use std::net::{IpAddr, Ipv4Addr};

use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};

/// Names of virtual adapters that phones in the venue cannot reach
/// (listed last, never dropped — the operator may still need them).
const VIRTUAL_HINTS: &[&str] = &[
    "docker",
    "veth",
    "vbox",
    "vmnet",
    "virtual",
    "wsl",
    "hyper-v",
    "utun",
    "tailscale",
    "zerotier",
    "bridge",
    "llw",
    "awdl",
];

fn is_virtual(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    VIRTUAL_HINTS.iter().any(|h| n.contains(h))
}

fn private_rank(ip: Ipv4Addr) -> u8 {
    let o = ip.octets();
    match o {
        [192, 168, ..] => 0,
        [10, ..] => 1,
        [172, b, ..] if (16..=31).contains(&b) => 2,
        _ => 3,
    }
}

/// Pure filter: non-loopback, non-link-local, unicast IPv4 addresses,
/// typical venue networks (192.168/16, 10/8, 172.16/12) first, virtual
/// adapters last; duplicates removed.
pub fn lan_ipv4(interfaces: &[(String, IpAddr)]) -> Vec<Ipv4Addr> {
    let mut out: Vec<(bool, u8, Ipv4Addr)> = Vec::new();
    for (name, ip) in interfaces {
        let IpAddr::V4(v4) = ip else { continue };
        if v4.is_loopback()
            || v4.is_link_local()
            || v4.is_unspecified()
            || v4.is_broadcast()
            || v4.is_multicast()
            || v4.is_documentation()
        {
            continue;
        }
        if out.iter().any(|(_, _, x)| x == v4) {
            continue;
        }
        out.push((is_virtual(name), private_rank(*v4), *v4));
    }
    out.sort();
    out.into_iter().map(|(_, _, ip)| ip).collect()
}

/// All LAN IPv4 addresses of this machine (see [`lan_ipv4`]).
pub fn list_lan_ipv4() -> Vec<Ipv4Addr> {
    match if_addrs::get_if_addrs() {
        Ok(ifaces) => {
            let pairs: Vec<(String, IpAddr)> = ifaces
                .into_iter()
                .map(|i| (i.name.clone(), i.ip()))
                .collect();
            lan_ipv4(&pairs)
        }
        Err(e) => {
            eprintln!("on-prem: network interfaces not readable: {e}");
            Vec::new()
        }
    }
}

fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `http://<host>:<port>/judge/?t=<token>&o=<owner_sub>`.
pub fn judge_url(host: &str, port: u16, token: &str, owner_sub: &str) -> String {
    format!(
        "http://{host}:{port}/judge/?t={}&o={}",
        encode_component(token),
        encode_component(owner_sub)
    )
}

/// QR code of `url` as a standalone SVG (black on white, quiet zone).
pub fn qr_svg(url: &str) -> Result<String, String> {
    let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::M)
        .map_err(|e| format!("QR-Code nicht erzeugbar: {e}"))?;
    Ok(code
        .render::<svg::Color<'_>>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iface(name: &str, ip: &str) -> (String, IpAddr) {
        (name.to_string(), ip.parse().unwrap())
    }

    #[test]
    fn filters_loopback_link_local_and_v6() {
        let ips = lan_ipv4(&[
            iface("lo0", "127.0.0.1"),
            iface("en0", "169.254.10.2"),
            iface("en0", "fe80::1"),
            iface("en0", "2001:db8::1"),
            iface("en0", "0.0.0.0"),
            iface("en0", "224.0.0.1"),
            iface("en1", "192.168.1.20"),
        ]);
        assert_eq!(ips, vec!["192.168.1.20".parse::<Ipv4Addr>().unwrap()]);
    }

    #[test]
    fn private_ranges_first_virtual_last() {
        let ips = lan_ipv4(&[
            iface("docker0", "172.17.0.1"),
            iface("eth1", "100.64.3.4"),
            iface("eth0", "10.0.0.5"),
            iface("wlan0", "192.168.178.30"),
            iface("vEthernet (WSL)", "192.168.80.1"),
            iface("eth2", "172.20.1.1"),
        ]);
        let s: Vec<String> = ips.iter().map(|i| i.to_string()).collect();
        assert_eq!(
            s,
            vec![
                "192.168.178.30",
                "10.0.0.5",
                "172.20.1.1",
                "100.64.3.4",
                "192.168.80.1",
                "172.17.0.1"
            ]
        );
    }

    #[test]
    fn duplicates_removed() {
        let ips = lan_ipv4(&[iface("en0", "192.168.1.2"), iface("en0", "192.168.1.2")]);
        assert_eq!(ips.len(), 1);
    }

    #[test]
    fn empty_input() {
        assert!(lan_ipv4(&[]).is_empty());
    }

    #[test]
    fn judge_url_format() {
        assert_eq!(
            judge_url("192.168.1.20", 8787, "0123abcd", "8a4b-uuid"),
            "http://192.168.1.20:8787/judge/?t=0123abcd&o=8a4b-uuid"
        );
        assert_eq!(
            judge_url("10.0.0.1", 8790, "t", "a b/c?"),
            "http://10.0.0.1:8790/judge/?t=t&o=a%20b%2Fc%3F"
        );
    }

    #[test]
    fn qr_is_svg_and_encodes_url() {
        let url = judge_url(
            "192.168.1.20",
            8787,
            "0123456789abcdef0123456789abcdef",
            "sub",
        );
        let svg = qr_svg(&url).unwrap();
        assert!(svg.starts_with("<?xml") || svg.starts_with("<svg"));
        assert!(svg.contains("<svg"));
        assert!(svg.contains("#000000"));
        assert!(svg.len() > 1000);
    }

    #[test]
    fn listing_real_interfaces_never_panics() {
        for ip in list_lan_ipv4() {
            assert!(!ip.is_loopback());
        }
    }
}
