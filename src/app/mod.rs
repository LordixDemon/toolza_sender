//! Модуль приложения - состояние и логика

mod state;
mod actions;
mod event_handler;

pub use state::{App, Mode, TargetStatus};
// DialogResult используется внутри модуля actions

