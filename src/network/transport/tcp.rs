//! TCP транспорт

use super::{TransportListener, TransportStream};
use async_trait::async_trait;
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};

/// TCP поток
pub struct TcpStreamWrapper {
    stream: TcpStream,
}

impl TcpStreamWrapper {
    pub fn new(stream: TcpStream) -> Self {
        // Отключаем алгоритм Нейгла для меньшей задержки
        stream.set_nodelay(true).ok();
        Self { stream }
    }
    
    /// Получить ссылку на внутренний TcpStream
    pub fn inner(&self) -> &TcpStream {
        &self.stream
    }
    
    /// Получить мутабельную ссылку на внутренний TcpStream
    pub fn inner_mut(&mut self) -> &mut TcpStream {
        &mut self.stream
    }
}

#[async_trait]
impl TransportStream for TcpStreamWrapper {
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

/// TCP слушатель
pub struct TcpListenerWrapper {
    listener: TcpListener,
}

#[async_trait]
impl TransportListener for TcpListenerWrapper {
    async fn accept(&mut self) -> io::Result<(Box<dyn TransportStream>, String)> {
        let (stream, addr) = self.listener.accept().await?;
        Ok((Box::new(TcpStreamWrapper::new(stream)), addr.to_string()))
    }
    
    async fn accept_timeout(&mut self, duration: Duration) -> io::Result<Option<(Box<dyn TransportStream>, String)>> {
        match timeout(duration, self.listener.accept()).await {
            Ok(Ok((stream, addr))) => {
                Ok(Some((Box::new(TcpStreamWrapper::new(stream)), addr.to_string())))
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(None), // Timeout
        }
    }
}

/// TCP транспорт
#[derive(Clone)]
pub struct TcpTransport;

impl TcpTransport {
    pub fn new() -> Self {
        Self
    }
    
    pub async fn connect(&self, addr: &str) -> io::Result<TcpStreamWrapper> {
        let stream = TcpStream::connect(addr).await?;
        Ok(TcpStreamWrapper::new(stream))
    }
    
    pub async fn bind(&self, port: u16) -> io::Result<TcpListenerWrapper> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        Ok(TcpListenerWrapper { listener })
    }
}

impl Default for TcpTransport {
    fn default() -> Self {
        Self::new()
    }
}
