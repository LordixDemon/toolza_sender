//! Сканирование локальной сети

use super::events::TransferEvent;
use crate::utils::get_local_ip;
use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

/// Подсеть для сканирования (первые 3 октета)
#[derive(Clone, Debug)]
pub struct Subnet {
    pub octets: [u8; 3],
}

impl Subnet {
    /// Создать подсеть из первых трёх октетов
    pub fn new(a: u8, b: u8, c: u8) -> Self {
        Self { octets: [a, b, c] }
    }
    
    /// Парсить строку подсети
    /// Поддерживаемые форматы:
    /// - "192.168.1" 
    /// - "192.168.1.0"
    /// - "192.168.1.0/24"
    /// - "192.168.1.x"
    pub fn parse(s: &str) -> Option<Self> {
        // Убираем /24 и подобные суффиксы
        let s = s.split('/').next().unwrap_or(s);
        
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() < 3 {
            return None;
        }
        
        let a: u8 = parts[0].parse().ok()?;
        let b: u8 = parts[1].parse().ok()?;
        let c: u8 = parts[2].parse().ok()?;
        
        Some(Self::new(a, b, c))
    }
    
    /// Получить базовый адрес как строку
    pub fn base(&self) -> String {
        format!("{}.{}.{}.", self.octets[0], self.octets[1], self.octets[2])
    }
    
    /// Получить полный IP адрес с последним октетом
    pub fn ip(&self, last: u8) -> Ipv4Addr {
        Ipv4Addr::new(self.octets[0], self.octets[1], self.octets[2], last)
    }
}

impl std::fmt::Display for Subnet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}.0/24", self.octets[0], self.octets[1], self.octets[2])
    }
}

/// Парсить список подсетей из строки (разделитель: запятая или пробел)
pub fn parse_subnets(input: &str) -> Vec<Subnet> {
    input
        .split(|c| c == ',' || c == ' ' || c == ';')
        .filter(|s| !s.is_empty())
        .filter_map(|s| Subnet::parse(s.trim()))
        .collect()
}

/// Сканировать подсеть на наличие серверов (автоопределение локальной сети)
pub async fn scan_network(
    port: u16,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
) -> Result<Vec<String>, String> {
    let local_ip = get_local_ip()
        .ok_or_else(|| "Не удалось определить локальный IP".to_string())?;
    
    let octets = local_ip.octets();
    let subnet = Subnet::new(octets[0], octets[1], octets[2]);
    
    scan_subnets(vec![subnet], port, event_tx).await
}

/// Сканировать указанные подсети на наличие серверов
pub async fn scan_subnets(
    subnets: Vec<Subnet>,
    port: u16,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
) -> Result<Vec<String>, String> {
    if subnets.is_empty() {
        return Err("Не указаны подсети для сканирования".to_string());
    }
    
    let total_subnets = subnets.len();
    let mut found_servers = Vec::new();
    let mut last_progress_update = std::time::Instant::now();
    
    for (subnet_idx, subnet) in subnets.iter().enumerate() {
        let base_ip = subnet.base();
        
        let _ = event_tx.send(TransferEvent::ScanProgress(
            format!("Подсеть {}/{}: {}0/24", subnet_idx + 1, total_subnets, base_ip),
            0,
        ));
        
        // Сканируем пакетами по 32 адреса
        let batch_size = 32;
        
        for batch_start in (1u8..255).step_by(batch_size) {
            let batch_end = (batch_start as usize + batch_size).min(255) as u8;
            
            let mut handles = Vec::new();
            
            for i in batch_start..batch_end {
                let ip = subnet.ip(i);
                let handle = tokio::spawn(check_server(ip, port));
                handles.push(handle);
            }
            
            // Собираем результаты пакета
            for handle in handles {
                if let Ok(Some(addr)) = handle.await {
                    found_servers.push(addr.clone());
                    let _ = event_tx.send(TransferEvent::ServerFound(addr));
                }
            }
            
            // Обновляем прогресс раз в секунду
            if last_progress_update.elapsed().as_secs() >= 1 {
                let subnet_progress = (batch_end as f32 / 254.0) * 100.0;
                let total_progress = ((subnet_idx as f32 + subnet_progress / 100.0) / total_subnets as f32 * 100.0) as u8;
                let _ = event_tx.send(TransferEvent::ScanProgress(
                    format!("{}{}", base_ip, batch_end),
                    total_progress,
                ));
                last_progress_update = std::time::Instant::now();
            }
        }
    }
    
    let _ = event_tx.send(TransferEvent::ScanCompleted);
    
    Ok(found_servers)
}

/// Проверить, доступен ли сервер на данном адресе
async fn check_server(ip: Ipv4Addr, port: u16) -> Option<String> {
    let addr = format!("{}:{}", ip, port);
    
    // Пробуем подключиться с коротким таймаутом
    let connect_future = TcpStream::connect(&addr);
    let timeout = tokio::time::timeout(Duration::from_millis(100), connect_future);
    
    match timeout.await {
        Ok(Ok(_stream)) => Some(addr),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // === Тесты Subnet ===
    
    #[test]
    fn test_subnet_new() {
        let subnet = Subnet::new(192, 168, 1);
        assert_eq!(subnet.octets, [192, 168, 1]);
    }
    
    #[test]
    fn test_subnet_parse_three_octets() {
        let subnet = Subnet::parse("192.168.1").unwrap();
        assert_eq!(subnet.octets, [192, 168, 1]);
    }
    
    #[test]
    fn test_subnet_parse_four_octets() {
        let subnet = Subnet::parse("192.168.1.0").unwrap();
        assert_eq!(subnet.octets, [192, 168, 1]);
        
        let subnet = Subnet::parse("10.0.0.255").unwrap();
        assert_eq!(subnet.octets, [10, 0, 0]);
    }
    
    #[test]
    fn test_subnet_parse_with_cidr() {
        let subnet = Subnet::parse("192.168.1.0/24").unwrap();
        assert_eq!(subnet.octets, [192, 168, 1]);
        
        let subnet = Subnet::parse("10.0.0.0/16").unwrap();
        assert_eq!(subnet.octets, [10, 0, 0]);
    }
    
    #[test]
    fn test_subnet_parse_invalid() {
        assert!(Subnet::parse("192.168").is_none());
        assert!(Subnet::parse("192").is_none());
        assert!(Subnet::parse("").is_none());
        assert!(Subnet::parse("abc.def.ghi").is_none());
        assert!(Subnet::parse("300.168.1").is_none()); // > 255
    }
    
    #[test]
    fn test_subnet_base() {
        let subnet = Subnet::new(192, 168, 1);
        assert_eq!(subnet.base(), "192.168.1.");
        
        let subnet = Subnet::new(10, 0, 0);
        assert_eq!(subnet.base(), "10.0.0.");
    }
    
    #[test]
    fn test_subnet_ip() {
        let subnet = Subnet::new(192, 168, 1);
        
        assert_eq!(subnet.ip(1), Ipv4Addr::new(192, 168, 1, 1));
        assert_eq!(subnet.ip(100), Ipv4Addr::new(192, 168, 1, 100));
        assert_eq!(subnet.ip(254), Ipv4Addr::new(192, 168, 1, 254));
    }
    
    #[test]
    fn test_subnet_display() {
        let subnet = Subnet::new(192, 168, 1);
        assert_eq!(format!("{}", subnet), "192.168.1.0/24");
        
        let subnet = Subnet::new(10, 0, 0);
        assert_eq!(format!("{}", subnet), "10.0.0.0/24");
    }
    
    // === Тесты parse_subnets ===
    
    #[test]
    fn test_parse_subnets_single() {
        let subnets = parse_subnets("192.168.1");
        assert_eq!(subnets.len(), 1);
        assert_eq!(subnets[0].octets, [192, 168, 1]);
    }
    
    #[test]
    fn test_parse_subnets_comma_separated() {
        let subnets = parse_subnets("192.168.1,10.0.0,172.16.0");
        assert_eq!(subnets.len(), 3);
        assert_eq!(subnets[0].octets, [192, 168, 1]);
        assert_eq!(subnets[1].octets, [10, 0, 0]);
        assert_eq!(subnets[2].octets, [172, 16, 0]);
    }
    
    #[test]
    fn test_parse_subnets_space_separated() {
        let subnets = parse_subnets("192.168.1 10.0.0");
        assert_eq!(subnets.len(), 2);
    }
    
    #[test]
    fn test_parse_subnets_semicolon_separated() {
        let subnets = parse_subnets("192.168.1;10.0.0");
        assert_eq!(subnets.len(), 2);
    }
    
    #[test]
    fn test_parse_subnets_mixed_separators() {
        let subnets = parse_subnets("192.168.1, 10.0.0; 172.16.0");
        assert_eq!(subnets.len(), 3);
    }
    
    #[test]
    fn test_parse_subnets_with_whitespace() {
        let subnets = parse_subnets("  192.168.1  ,  10.0.0  ");
        assert_eq!(subnets.len(), 2);
    }
    
    #[test]
    fn test_parse_subnets_empty() {
        let subnets = parse_subnets("");
        assert!(subnets.is_empty());
    }
    
    #[test]
    fn test_parse_subnets_invalid_skipped() {
        let subnets = parse_subnets("192.168.1,invalid,10.0.0");
        assert_eq!(subnets.len(), 2); // Невалидная подсеть пропущена
    }
}

