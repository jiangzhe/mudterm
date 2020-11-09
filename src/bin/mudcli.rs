use crossbeam_channel::unbounded;
use gag::Redirect;
use mudterm::app::client::{Client, ClientCallback};
use mudterm::auth;
use mudterm::conf::{CmdOpts, Config};
use mudterm::error::{Error, Result};
use mudterm::ui::{init_terminal, RawScreen};
use std::fs::File;
use std::io::Read;
use std::net::TcpStream;
use std::path::Path;
use structopt::StructOpt;

fn main() -> Result<()> {
    let cmdopts = CmdOpts::from_args();

    if !Path::new(&cmdopts.conf_file).exists() {
        return Err(Error::RuntimeError(format!(
            "config file {} not found",
            &cmdopts.conf_file
        )));
    }
    let config: Config = {
        let mut f = File::open(&cmdopts.conf_file)?;
        let mut toml_str = String::new();
        f.read_to_string(&mut toml_str)?;
        toml::from_str(&toml_str)?
    };

    // redirect stderr to file
    let debuglog = File::create(&config.client.debug_file)?;
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;

    // connect to server
    let (from_server, to_server) = {
        let from_server = TcpStream::connect(&config.client.server_addr)?;
        let from_server = auth::client_auth(from_server, &config.client.server_pass)?;
        let to_server = from_server.try_clone()?;
        (from_server, to_server)
    };

    // create client app
    let mut screen = RawScreen::new(config.term);

    let (uitx, uirx) = unbounded();

    // todo: init script

    let mut client = Client::new(uitx);

    client.start_from_server_handle(from_server);

    let srvtx = client.start_to_server_handle(to_server)?;

    client.start_signal_handle();

    client.start_userinput_handle();

    let mut terminal = init_terminal()?;
    
    let mut cb = ClientCallback::new(srvtx);
    
    loop {
        if screen.render(&mut terminal, &uirx, &mut cb)? {
            break;
        }
    }
    Ok(())
}
