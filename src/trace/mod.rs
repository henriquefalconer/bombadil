use std::{fmt::Display, path::PathBuf, time::SystemTime};

use serde::Serialize;
use url::Url;

use crate::browser::actions::BrowserAction;

pub mod writer;

#[derive(Debug, Clone, Serialize)]
pub struct TraceEntry {
    pub timestamp: SystemTime,
    pub url: Url,
    pub hash_previous: Option<u64>,
    pub hash_current: Option<u64>,
    pub action: Option<BrowserAction>,
    pub screenshot: PathBuf,
    pub violation: Option<Violation>,
}

#[derive(Clone, Debug, Serialize)]
pub enum Violation {
    Invariant(String),
    Unknown(String),
}

impl<E: Into<anyhow::Error>> From<E> for Violation {
    fn from(value: E) -> Self {
        Violation::Unknown(value.into().to_string())
    }
}

impl Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Violation::Invariant(message) => {
                write!(f, "invariant: {}", message)
            }
            Violation::Unknown(error) => {
                write!(f, "{}", error)
            }
        }
    }
}

#[macro_export]
macro_rules! invariant_violation {
    ($msg:literal $(,)?) => {
        return Result::Err(Violation::Invariant(format!("{}", $msg)))
    };
    ($err:expr $(,)?) => {
        return Result::Err(Violation::Invariant(format!("{}", $err)))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Result::Err(Violation::Invariant(format!($fmt, $($arg)*)))
    };
}
