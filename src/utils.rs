//! Вспомогательные утилиты

/// Безопасно обрезает строку до max_chars символов (не байт!) с начала
/// Если строка длиннее - показывает "..." и конец строки
pub fn truncate_string(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let skip = char_count.saturating_sub(max_chars.saturating_sub(3));
        format!("...{}", s.chars().skip(skip).collect::<String>())
    }
}

/// Форматирование размера файла в человекочитаемый вид
pub fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    
    if size >= GB {
        format!("{:.2} ГБ", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} МБ", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} КБ", size as f64 / KB as f64)
    } else {
        format!("{} Б", size)
    }
}

/// Получить локальный IP адрес
pub fn get_local_ip() -> Option<std::net::Ipv4Addr> {
    local_ip_address::local_ip()
        .ok()
        .and_then(|ip| match ip {
            std::net::IpAddr::V4(v4) => Some(v4),
            _ => None,
        })
}

/// Получить локальный IP как строку
pub fn get_local_ip_string() -> String {
    get_local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "Не определён".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0 Б");
        assert_eq!(format_size(1), "1 Б");
        assert_eq!(format_size(100), "100 Б");
        assert_eq!(format_size(1023), "1023 Б");
    }
    
    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.00 КБ");
        assert_eq!(format_size(1536), "1.50 КБ");
        assert_eq!(format_size(10240), "10.00 КБ");
    }
    
    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.00 МБ");
        assert_eq!(format_size(1024 * 1024 * 5), "5.00 МБ");
        assert_eq!(format_size(1024 * 1024 + 512 * 1024), "1.50 МБ");
    }
    
    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 ГБ");
        assert_eq!(format_size(1024 * 1024 * 1024 * 2), "2.00 ГБ");
        assert_eq!(format_size(1024u64 * 1024 * 1024 * 100), "100.00 ГБ");
    }
    
    #[test]
    fn test_get_local_ip_returns_valid_or_none() {
        // Этот тест просто проверяет что функция не паникует
        let ip = get_local_ip();
        if let Some(addr) = ip {
            // Если IP найден, он должен быть валидным IPv4
            assert!(!addr.is_unspecified());
        }
    }
    
    #[test]
    fn test_get_local_ip_string_returns_string() {
        let ip_str = get_local_ip_string();
        assert!(!ip_str.is_empty());
        // Либо IP адрес, либо "Не определён"
        assert!(ip_str.contains('.') || ip_str == "Не определён");
    }
}

