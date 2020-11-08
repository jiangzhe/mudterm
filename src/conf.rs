use serde::{Serialize, Deserialize};
use structopt::StructOpt;
// #[derive(Debug, Clone)]
// pub struct Config {
//     pub world_addr: String,
//     pub max_lines: usize,
//     pub server_log_file: String,
//     pub server_log_ansi: bool,
//     pub debug_log_file: String,
//     pub triggers_file: String,
//     pub echo_command: bool,
//     // pub log_command: bool,
// }

// impl Default for Config {
//     fn default() -> Self {
//         Self{
//             world_addr: String::from("mud.pkuxkx.net"),
//             max_lines: 5000,
//             server_log_file: String::from("server.log"),
//             server_log_ansi: true,
//             debug_log_file: String::from("debug.log"),
//             triggers_file: String::from("triggers.xml"),
//             echo_command: true,
//             // log_command: false,
//         }
//     }
// }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub world: World,
    pub server: Server,
    pub client: Client,
    pub term: Term,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub addr: String,
}

impl Default for World {
    fn default() -> Self {
        Self{
            addr: String::from("mud.pkuxkx.net:8080"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub port: u16,
    pub log_file: String,
    pub log_ansi: bool,
    pub debug_file: String,
    pub client_init_max_lines: usize,
    pub pass: String,
}

impl Default for Server {
    fn default() -> Self {
        Self{
            port: 9680,
            log_file: String::from("server.log"),
            log_ansi: false,
            debug_file: String::from("debug.log"),
            client_init_max_lines: 100,
            pass: String::from("pass"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub server_addr: String,
    pub server_pass: String,
    pub debug_file: String,
}

impl Default for Client {
    fn default() -> Self {
        Self{
            server_addr: String::from("127.0.0.1:9680"),
            server_pass: String::from("pass"),
            debug_file: String::from("client_debug.log"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Term {
    pub max_lines: usize,
    pub echo_cmd: bool,
    pub cmd_delimiter: char,
    pub ignore_empty_cmd: bool,
}

impl Default for Term {
    fn default() -> Self {
        Self{
            max_lines: 1000,
            echo_cmd: false,
            cmd_delimiter: ';',
            ignore_empty_cmd: true,
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, StructOpt)]
pub struct CmdOpts {
    #[structopt(short, long, default_value = "mud.toml")]
    pub conf_file: String,
}


#[cfg(test)]
mod tests {

    #[test]
    fn test_toml_deserialize_char() {
        let mut m = std::collections::HashMap::<String, char>::new();
        m.insert(String::from("a"), ';');
        let s = toml::to_string(&m).unwrap();
        println!("{}", s);
    }
}