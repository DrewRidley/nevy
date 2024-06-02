#[derive(Debug)]
pub struct WebError {
    value: JsValue,
}

impl From<JsValue> for WebError {
    fn from(value: JsValue) -> Self {
        Self { value }
    }
}

impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print out the JsValue as a string
        match self.value.as_string() {
            Some(s) => write!(f, "{}", s),
            None => write!(f, "{:?}", self.value),
        }
    }
}

impl From<&str> for WebError {
    fn from(value: &str) -> Self {
        Self {
            value: value.into(),
        }
    }
}
