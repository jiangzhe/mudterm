use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Connection error {0}")]
    ConnectionError(#[from] std::io::Error),
    #[error("Lua error {0}")]
    LuaError(#[from] mlua::Error),
    #[error("Receive error {0}")]
    RecvError(#[from] crossbeam_channel::RecvError),
    #[error("Send error {0}")]
    SendError(String),
    #[error("Unsupported target {0}")]
    UnsupportedTarget(String),
    #[error("Parse error {0}")]
    ParseError(String),
    #[error("Decode error {0}")]
    DecodeError(String),
    #[error("Encode error {0}")]
    EncodeError(String),
    #[error("Escape error {0}")]
    EscapeError(String),
    #[error("UTF8 error {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
    #[error("Regexp error {0}")]
    RegexpError(#[from] regex::Error),
    #[error("Runtime error {0}")]
    RuntimeError(String),
    #[error("Packet error {0}")]
    PacketError(String),
    #[error("Toml error {0}")]
    TomlError(#[from] toml::de::Error),
    #[error("Auth error")]
    AuthError,
    #[error("Compile script error {0}")]
    CompileScriptError(String),
}

impl<T> From<crossbeam_channel::SendError<T>> for Error {
    fn from(err: crossbeam_channel::SendError<T>) -> Self {
        Self::SendError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
