//! Сетевой модуль - отправка, приём и сканирование

pub mod compression;
pub mod transport;
pub mod sender;
pub mod receiver;
mod events;
mod scanner;
pub mod speedtest;

pub use events::TransferEvent;
pub use sender::{send_files_to_multiple, send_files_to_multiple_with_stop, SendOptions};
pub use receiver::{run_server, run_server_with_stop, run_server_with_options_and_stop, ServerOptions, ExtractOptions};
pub use scanner::{scan_network, scan_subnets, parse_subnets, Subnet};
pub use speedtest::{run_speedtest, SpeedTestResult, DEFAULT_SPEEDTEST_SIZE};
pub use transport::TransportType;

