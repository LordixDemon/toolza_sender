//! QUIC транспорт (требует feature "quic")

use super::{TransportListener, TransportStream};
use async_trait::async_trait;
use quinn::{ClientConfig, Endpoint, ServerConfig, RecvStream, SendStream};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

/// QUIC поток (пара send + recv)
pub struct QuicStreamWrapper {
    send: SendStream,
    recv: RecvStream,
}

impl QuicStreamWrapper {
    pub fn new(send: SendStream, recv: RecvStream) -> Self {
        Self { send, recv }
    }
}

#[async_trait]
impl TransportStream for QuicStreamWrapper {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv.read(buf).await
            .map(|opt| opt.unwrap_or(0))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
    
    async fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.recv.read_exact(buf).await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
    
    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.send.write_all(buf).await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
    
    async fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    
    async fn shutdown(&mut self) -> io::Result<()> {
        self.send.finish()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

/// QUIC слушатель
pub struct QuicListenerWrapper {
    endpoint: Endpoint,
}

#[async_trait]
impl TransportListener for QuicListenerWrapper {
    async fn accept(&mut self) -> io::Result<(Box<dyn TransportStream>, String)> {
        let incoming = self.endpoint.accept().await
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Endpoint closed"))?;
        
        let connection = incoming.await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        let addr = connection.remote_address().to_string();
        
        let (send, recv) = connection.accept_bi().await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        Ok((Box::new(QuicStreamWrapper::new(send, recv)), addr))
    }
    
    async fn accept_timeout(&mut self, duration: Duration) -> io::Result<Option<(Box<dyn TransportStream>, String)>> {
        match timeout(duration, self.accept()).await {
            Ok(result) => result.map(Some),
            Err(_) => Ok(None),
        }
    }
}

/// QUIC транспорт
pub struct QuicTransport {
    client_config: ClientConfig,
}

impl QuicTransport {
    pub fn new() -> Self {
        let client_config = configure_client();
        Self { client_config }
    }
    
    pub async fn connect(&self, addr: &str) -> io::Result<QuicStreamWrapper> {
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        endpoint.set_default_client_config(self.client_config.clone());
        
        let socket_addr: SocketAddr = addr.parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        
        let connection = endpoint.connect(socket_addr, "toolza")
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        let (send, recv) = connection.open_bi().await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        Ok(QuicStreamWrapper::new(send, recv))
    }
    
    pub async fn bind(&self, port: u16) -> io::Result<QuicListenerWrapper> {
        let (server_config, _cert) = configure_server()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
        let endpoint = Endpoint::server(server_config, addr)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        Ok(QuicListenerWrapper { endpoint })
    }
}

impl Default for QuicTransport {
    fn default() -> Self {
        Self::new()
    }
}

fn configure_client() -> ClientConfig {
    let crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
        .with_no_client_auth();
    
    ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto).unwrap()
    ))
}

fn configure_server() -> Result<(ServerConfig, Vec<u8>), Box<dyn std::error::Error + Send + Sync>> {
    let cert = rcgen::generate_simple_self_signed(vec!["toolza".to_string()])?;
    let cert_der = cert.cert.der().to_vec();
    let key_der = cert.key_pair.serialize_der();
    
    let cert_chain = vec![rustls::pki_types::CertificateDer::from(cert_der.clone())];
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(key_der);
    
    let server_crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key.into())?;
    
    let server_config = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)?
    ));
    
    Ok((server_config, cert_der))
}

#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
        ]
    }
}
