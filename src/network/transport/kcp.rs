//! KCP транспорт - быстрый надёжный UDP (требует feature "kcp")

use super::{TransportListener, TransportStream};
use async_trait::async_trait;
use std::io;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};
use tokio_kcp::{KcpConfig, KcpListener, KcpStream};

/// KCP поток
pub struct KcpStreamWrapper {
    stream: KcpStream,
}

impl KcpStreamWrapper {
    pub fn new(stream: KcpStream) -> Self {
        Self { stream }
    }
}

#[async_trait]
impl TransportStream for KcpStreamWrapper {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf).await
    }
    
    async fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.stream.read_exact(buf).await?;
        Ok(())
    }
    
    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.stream.write_all(buf).await
    }
    
    async fn flush(&mut self) -> io::Result<()> {
        self.stream.flush().await
    }
    
    async fn shutdown(&mut self) -> io::Result<()> {
        self.stream.shutdown().await
    }
}

/// KCP слушатель
pub struct KcpListenerWrapper {
    listener: KcpListener,
}

#[async_trait]
impl TransportListener for KcpListenerWrapper {
    async fn accept(&mut self) -> io::Result<(Box<dyn TransportStream>, String)> {
        let (stream, addr) = self.listener.accept().await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        Ok((Box::new(KcpStreamWrapper::new(stream)), addr.to_string()))
    }
    
    async fn accept_timeout(&mut self, duration: Duration) -> io::Result<Option<(Box<dyn TransportStream>, String)>> {
        match timeout(duration, self.listener.accept()).await {
            Ok(Ok((stream, addr))) => {
                Ok(Some((Box::new(KcpStreamWrapper::new(stream)), addr.to_string())))
            }
            Ok(Err(e)) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
            Err(_) => Ok(None),
        }
    }
}

/// KCP транспорт
pub struct KcpTransport {
    config: KcpConfig,
}

impl KcpTransport {
    pub fn new() -> Self {
        // Оптимальная конфигурация для передачи файлов
        let mut config = KcpConfig::default();
        config.mtu = 1400;
        config.nodelay = tokio_kcp::KcpNoDelayConfig::fastest();
        config.wnd_size = (1024, 1024); // Большое окно для высокой пропускной способности
        config.stream = true;
        
        Self { config }
    }
    
    pub async fn connect(&self, addr: &str) -> io::Result<KcpStreamWrapper> {
        let socket_addr: SocketAddr = addr.parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        
        let stream = KcpStream::connect(&self.config, socket_addr).await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        Ok(KcpStreamWrapper::new(stream))
    }
    
    pub async fn bind(&self, port: u16) -> io::Result<KcpListenerWrapper> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
        let listener = KcpListener::bind(self.config.clone(), addr).await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        Ok(KcpListenerWrapper { listener })
    }
}

impl Default for KcpTransport {
    fn default() -> Self {
        Self::new()
    }
}

