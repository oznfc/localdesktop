use crate::utils::config;
use std::{collections::VecDeque, sync::Mutex};

pub fn log_to_panel(content: &str, log_panel: &Mutex<VecDeque<String>>) {
    let mut logs = log_panel.lock().unwrap();
    logs.push_back(content.to_string());
    // Ensure the logs size stays at most 20
    if logs.len() > config::MAX_PANEL_LOG_ENTRIES {
        logs.pop_front();
    }
}

pub fn log_format(title: &str, content: &str) -> String {
    format!(
        "\n*** *** *** [{}] *** *** ***\n{}\n*** *** *** [{}] *** *** ***\n\n",
        title, content, title
    )
}

pub trait PolarBearExpectation<T> {
    fn pb_expect(self, msg: &str) -> T;
}

impl<T, E> PolarBearExpectation<T> for Result<T, E>
where
    E: std::fmt::Debug,
{
    fn pb_expect(self, msg: &str) -> T {
        self.expect(&log_format("POLAR BEAR EXPECTATION", msg))
    }
}

impl<T> PolarBearExpectation<T> for Option<T> {
    fn pb_expect(self, msg: &str) -> T {
        self.expect(&log_format("POLAR BEAR EXPECTATION", msg))
    }
}
