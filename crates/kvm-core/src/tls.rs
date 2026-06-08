/// Self-signed TLS per install + trust-on-first-connect fingerprint pairing.
use anyhow::Result;
use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ServerConfig};
use std::path::Path;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

pub struct TlsFiles {
    pub cert_pem: String,
    pub key_pem: String,
}

/// Generate a fresh self-signed cert/key pair for this install.
pub fn generate_self_signed(subject: &str) -> Result<TlsFiles> {
    let cert = generate_simple_self_signed(vec![subject.to_string()])?;
    Ok(TlsFiles {
        cert_pem: cert.serialize_pem()?,
        key_pem: cert.serialize_private_key_pem(),
    })
}

/// Compute a hex fingerprint (SHA-256) of a DER certificate.
pub fn fingerprint(cert_der: &[u8]) -> String {
    use std::fmt::Write;
    // Simple SHA-256 without a heavy dep: use rustls's built-in hasher via ring.
    // Fallback: use a basic hash for now (ring is pulled in via rustls).
    let digest = ring_sha256(cert_der);
    let mut s = String::with_capacity(64);
    for b in &digest {
        write!(s, "{:02x}", b).unwrap();
    }
    s
}

fn ring_sha256(data: &[u8]) -> Vec<u8> {
    // Use ring directly (already a transitive dep via rustls/ring feature).
    use rustls::crypto::ring;
    let _algo = ring::default_provider();
    // Compute via std SHA-256 fallback using pure Rust.
    sha256_pure(data)
}

/// Pure-Rust SHA-256. Avoids pulling in an extra crate.
fn sha256_pure(data: &[u8]) -> Vec<u8> {
    // Initial hash values (first 32 bits of fractional parts of sqrt of first 8 primes).
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    // Round constants.
    let k: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    // Pre-processing: pad the message
    let mut msg = data.to_vec();
    let orig_len_bits = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&orig_len_bits.to_be_bytes());

    // Process each 512-bit block
    for block in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(k[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g; g = f; f = e;
            e = d.wrapping_add(temp1);
            d = c; c = b; b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a); h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c); h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e); h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g); h[7] = h[7].wrapping_add(hh);
    }

    let mut out = vec![0u8; 32];
    for (i, &val) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&val.to_be_bytes());
    }
    out
}

/// Load cert and key PEM files from disk.
pub fn load_tls_files(cert_path: &Path, key_path: &Path) -> Result<TlsFiles> {
    Ok(TlsFiles {
        cert_pem: std::fs::read_to_string(cert_path)?,
        key_pem: std::fs::read_to_string(key_path)?,
    })
}

/// Build a TLS server config from PEM cert+key.
pub fn server_config(files: &TlsFiles) -> Result<Arc<ServerConfig>> {
    let cert_der = rustls_pemfile::certs(&mut files.cert_pem.as_bytes())
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let key_der = rustls_pemfile::private_key(&mut files.key_pem.as_bytes())?
        .ok_or_else(|| anyhow::anyhow!("no private key found"))?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_der, key_der)?;
    Ok(Arc::new(config))
}

/// Build a TLS client config that accepts any server cert (TOFU).
/// Caller verifies the fingerprint after connect.
pub fn client_config_tofu() -> Arc<ClientConfig> {
    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_no_client_auth();
    Arc::new(config)
}

/// Wrap an accepted TcpStream as a TLS server stream.
pub async fn wrap_server(
    stream: TcpStream,
    config: Arc<ServerConfig>,
) -> Result<tokio_rustls::server::TlsStream<TcpStream>> {
    Ok(TlsAcceptor::from(config).accept(stream).await?)
}

/// Wrap an outgoing TcpStream as a TLS client stream.
pub async fn wrap_client(
    stream: TcpStream,
    config: Arc<ClientConfig>,
    server_name: &str,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
    let name = ServerName::try_from(server_name.to_string())?;
    Ok(TlsConnector::from(config).connect(name, stream).await?)
}

/// Extract the peer cert DER from a TLS server stream and return its fingerprint.
pub fn server_stream_fingerprint(
    stream: &tokio_rustls::server::TlsStream<TcpStream>,
) -> Option<String> {
    let (_, session) = stream.get_ref();
    session
        .peer_certificates()
        .and_then(|certs| certs.first())
        .map(|c| fingerprint(c.as_ref()))
}

/// Extract the peer cert DER from a TLS client stream and return its fingerprint.
pub fn client_stream_fingerprint(
    stream: &tokio_rustls::client::TlsStream<TcpStream>,
) -> Option<String> {
    let (_, session) = stream.get_ref();
    session
        .peer_certificates()
        .and_then(|certs| certs.first())
        .map(|c| fingerprint(c.as_ref()))
}

// Custom verifier that accepts any certificate (TOFU model).
#[derive(Debug)]
struct AcceptAnyCert;

impl rustls::client::danger::ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_cert_produces_pem() {
        let files = generate_self_signed("hopper.local").unwrap();
        assert!(files.cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(files.key_pem.contains("PRIVATE KEY"));
    }

    #[test]
    fn fingerprint_deterministic() {
        let data = b"hopper fingerprint test";
        let fp1 = fingerprint(data);
        let fp2 = fingerprint(data);
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 64); // 32 bytes hex
    }

    #[test]
    fn fingerprint_differs_for_different_input() {
        let fp1 = fingerprint(b"cert-a");
        let fp2 = fingerprint(b"cert-b");
        assert_ne!(fp1, fp2);
    }

    #[tokio::test]
    async fn tls_handshake_loopback() {
        // rustls requires a CryptoProvider to be installed before use.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let files = generate_self_signed("localhost").unwrap();
        let srv_cfg = server_config(&files).unwrap();
        let cli_cfg = client_config_tofu();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let srv_cfg_clone = srv_cfg.clone();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let tls = wrap_server(tcp, srv_cfg_clone).await.unwrap();
            let fp = server_stream_fingerprint(&tls);
            fp
        });

        let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
        let tls = wrap_client(tcp, cli_cfg, "localhost").await.unwrap();
        let client_fp = client_stream_fingerprint(&tls);

        let server_fp = server.await.unwrap();

        // Both sides see a fingerprint; client sees server cert, server has no client cert.
        assert!(client_fp.is_some(), "client should see server cert fingerprint");
        assert_eq!(client_fp.unwrap().len(), 64);
        // server_fp is None because we use no_client_auth
        assert!(server_fp.is_none());
    }
}
