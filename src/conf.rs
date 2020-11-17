use serde::{Deserialize, Serialize};
use structopt::StructOpt;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub mode: Mode,
    pub world: World,
    pub server: Server,
    pub client: Client,
    pub term: Term,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct World {
    pub addr: String,
}

impl Default for World {
    fn default() -> Self {
        Self {
            addr: String::from("mud.pkuxkx.net:8080"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Server {
    pub port: u16,
    pub log_file: String,
    pub debug_file: String,
    pub client_init_max_lines: usize,
    pub pass: String,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            port: 9680,
            log_file: String::from("server.log"),
            debug_file: String::from("debug.log"),
            client_init_max_lines: 100,
            pass: String::from("pass"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Client {
    pub server_addr: String,
    pub server_pass: String,
    pub log_file: String,
    pub debug_file: String,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            server_addr: String::from("127.0.0.1:9680"),
            server_pass: String::from("pass"),
            log_file: String::from("client.log"),
            debug_file: String::from("client_debug.log"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Term {
    pub max_lines: usize,
    pub echo_cmd: bool,
    pub cmd_delim: char,
    pub send_empty_cmd: bool,
    pub reserve_cr: bool,
    pub pad_non_cjk: bool,
}

impl Default for Term {
    fn default() -> Self {
        Self {
            max_lines: 1000,
            echo_cmd: false,
            cmd_delim: ';',
            send_empty_cmd: false,
            reserve_cr: false,
            pad_non_cjk: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, StructOpt)]
pub struct CmdOpts {
    #[structopt(short, long, default_value = "mud.toml")]
    pub conf_file: String,
    #[structopt(short, long, default_value = "info")]
    pub log_level: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Mode {
    #[serde(rename = "standalone")]
    Standalone,
    #[serde(rename = "server")]
    Server,
    #[serde(rename = "client")]
    Client,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Standalone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toml_deserialize_char() {
        let mut m = std::collections::HashMap::<String, char>::new();
        m.insert(String::from("a"), ';');
        let s = toml::to_string(&m).unwrap();
        println!("{}", s);
    }

    #[test]
    fn test_toml_serialize_enum() {
        let m = Mode::Standalone;
        let s = toml::to_string(&m).unwrap();
        println!("{}", s);
    }
}
