use std::fmt;

#[derive(Debug)]
pub struct Failure {
    pub reason: &'static str,
    pub message: String,
    pub exit: i32,
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for Failure {}
pub fn fail<T>(exit: i32, reason: &'static str, message: impl Into<String>) -> anyhow::Result<T> {
    Err(Failure {
        exit,
        reason,
        message: message.into(),
    }
    .into())
}
pub fn config<T>(reason: &'static str, message: impl Into<String>) -> anyhow::Result<T> {
    fail(4, reason, message)
}
