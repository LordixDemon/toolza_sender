//! LZ4 сжатие для быстрой передачи

use lz4_flex::{compress_prepend_size, decompress_size_prepended};

/// Сжать данные с LZ4
pub fn compress(data: &[u8]) -> Vec<u8> {
    compress_prepend_size(data)
}

/// Распаковать LZ4 данные
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    decompress_size_prepended(data)
        .map_err(|e| format!("Ошибка распаковки: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compress_decompress() {
        let original = b"Hello, World! This is a test of LZ4 compression.";
        let compressed = compress(original);
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original.as_slice(), decompressed.as_slice());
    }
}

