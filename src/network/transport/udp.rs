//! UDP транспорт - быстрый, но без гарантии доставки
//! ⚠️ Не рекомендуется для больших файлов - возможна потеря пакетов!

use super::{TransportListener, TransportStream};
use async_trait::async_trait;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

/// Размер буфера для UDP пакетов
const UDP_BUFFER_SIZE: usize = 65507; // Максимальный размер UDP датаграммы

/// UDP "соединение" (на самом деле пара сокетов)
pub struct UdpStreamWrapper {
    socket: Arc<UdpSocket>,
    peer_addr: SocketAddr,
    recv_buffer: Vec<u8>,
    recv_pos: usize,
    recv_len: usize,
}

impl UdpStreamWrapper {
    pub fn new(socket: Arc<UdpSocket>, peer_addr: SocketAddr) -> Self {
        Self {
            socket,
            peer_addr,
            recv_buffer: vec![0u8; UDP_BUFFER_SIZE],
            recv_pos: 0,
            recv_len: 0,
        }
    }
    
    /// Подключиться к UDP серверу
    pub async fn connect(addr: &str) -> io::Result<Self> {
        let socket_addr: SocketAddr = addr.parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        
        // Создаём локальный сокет на случайном порту
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(socket_addr).await?;
        
        Ok(Self {
            socket: Arc::new(socket),
            peer_addr: socket_addr,
            recv_buffer: vec![0u8; UDP_BUFFER_SIZE],
            recv_pos: 0,
            recv_len: 0,
        })
    }
}

#[async_trait]
impl TransportStream for UdpStreamWrapper {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Если в буфере есть данные - читаем из него
        if self.recv_pos < self.recv_len {
            let available = self.recv_len - self.recv_pos;
            let to_copy = buf.len().min(available);
            buf[..to_copy].copy_from_slice(&self.recv_buffer[self.recv_pos..self.recv_pos + to_copy]);
            self.recv_pos += to_copy;
            return Ok(to_copy);
        }
        
        // Иначе читаем новый пакет
        let (len, _addr) = self.socket.recv_from(&mut self.recv_buffer).await?;
        
        let to_copy = buf.len().min(len);
        buf[..to_copy].copy_from_slice(&self.recv_buffer[..to_copy]);
        
        if to_copy < len {
            // Сохраняем остаток в буфере
            self.recv_pos = to_copy;
            self.recv_len = len;
        } else {
            self.recv_pos = 0;
            self.recv_len = 0;
        }
        
        Ok(to_copy)
    }
    
    async fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let mut total_read = 0;
        while total_read < buf.len() {
            let n = self.read(&mut buf[total_read..]).await?;
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "UDP connection closed"));
            }
            total_read += n;
        }
        Ok(())
    }
    
    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        // UDP ограничен размером датаграммы, отправляем чанками
        const MAX_CHUNK: usize = 65000;
        
        for chunk in buf.chunks(MAX_CHUNK) {
            self.socket.send_to(chunk, self.peer_addr).await?;
        }
        Ok(())
    }
    
    async fn flush(&mut self) -> io::Result<()> {
        // UDP не буферизирует
        Ok(())
    }
    
    async fn shutdown(&mut self) -> io::Result<()> {
        // UDP не имеет shutdown, просто ничего не делаем
        Ok(())
    }
}

/// UDP слушатель
pub struct UdpListenerWrapper {
    socket: Arc<UdpSocket>,
    #[allow(dead_code)]
    pending_clients: Arc<Mutex<Vec<(SocketAddr, Vec<u8>)>>>,
}

impl UdpListenerWrapper {
    pub async fn bind(port: u16) -> io::Result<Self> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
        let socket = UdpSocket::bind(addr).await?;
        
        Ok(Self {
            socket: Arc::new(socket),
            pending_clients: Arc::new(Mutex::new(Vec::new())),
        })
    }
}

#[async_trait]
impl TransportListener for UdpListenerWrapper {
    async fn accept(&mut self) -> io::Result<(Box<dyn TransportStream>, String)> {
        // В UDP нет понятия "accept" - просто ждём первый пакет от нового клиента
        let mut buf = vec![0u8; UDP_BUFFER_SIZE];
        let (len, addr) = self.socket.recv_from(&mut buf).await?;
        buf.truncate(len);
        
        // Создаём "соединение" с этим клиентом
        let mut wrapper = UdpStreamWrapper::new(self.socket.clone(), addr);
        // Сохраняем первый пакет в буфер
        wrapper.recv_buffer[..len].copy_from_slice(&buf);
        wrapper.recv_len = len;
        wrapper.recv_pos = 0;
        
        Ok((Box::new(wrapper), addr.to_string()))
    }
    
    async fn accept_timeout(&mut self, duration: Duration) -> io::Result<Option<(Box<dyn TransportStream>, String)>> {
        match timeout(duration, self.accept()).await {
            Ok(result) => result.map(Some),
            Err(_) => Ok(None),
        }
    }
}

/// UDP транспорт
pub struct UdpTransport;

impl UdpTransport {
    pub fn new() -> Self {
        Self
    }
    
    pub async fn connect(addr: &str) -> io::Result<UdpStreamWrapper> {
        UdpStreamWrapper::connect(addr).await
    }
    
    pub async fn bind(port: u16) -> io::Result<UdpListenerWrapper> {
        UdpListenerWrapper::bind(port).await
    }
}

impl Default for UdpTransport {
    fn default() -> Self {
        Self::new()
    }
}

