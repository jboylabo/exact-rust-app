//! Exact Rust App's data source: the view asks `faceFor(angel)` and Rust
//! answers which face to show and what the button says.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source.
#[derive(Default, Clone, Copy)]
pub struct Toggle;

/// A `Face` record, its fields in the order `shape Face` declares them: emoji, label.
fn face(angel: bool) -> Value {
    if angel {
        Value::record(vec![Value::str("😇"), Value::str("太陽にする")])
    } else {
        Value::record(vec![Value::str("☀️"), Value::str("天使にする")])
    }
}

impl DataSource for Toggle {
    fn app_id(&self) -> &str {
        "com.example.exact-rust-app-2d7a17"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match (source, args) {
            ("faceFor", [Value::Bool(angel)]) => Ok(face(*angel)),
            ("faceFor", _) => Err(DataError::Unavailable("faceFor takes one bool".into())),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angel_and_sun_alternate() {
        let mut data = Toggle;
        let angel = data.query("faceFor", &[Value::Bool(true)]).unwrap();
        let sun = data.query("faceFor", &[Value::Bool(false)]).unwrap();
        assert_eq!(angel, face(true));
        assert_eq!(sun, face(false));
        assert_ne!(angel, sun);
    }

    #[test]
    fn unknown_source_is_an_error() {
        assert!(Toggle.query("nope", &[]).is_err());
    }
}
