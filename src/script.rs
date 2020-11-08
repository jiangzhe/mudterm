use rlua::Lua;
use crate::error::Result;
use crate::codec::Codec;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock, Mutex};
use crate::style::StyledLine;
use tui::style::{Style, Color};
use tui::text::Span;
use crate::event::DerivedEvent;
use regex::Regex;
use serde::{Serialize, Deserialize};

pub struct Script {
    pub(crate) lua: Lua,
}

impl Script {
    pub fn new() -> Self {
        let lua = Lua::new();
        Self{lua}
    }

    pub fn exec<T: AsRef<[u8]>>(&self, input: T) -> Result<()> {
        let input = input.as_ref();
        let output = self.lua.context::<_, rlua::Result<()>>(move |lua_ctx| {
            let rst = lua_ctx.load(input).exec()?;
            Ok(rst)
        })?;
        Ok(output)
    }

    pub fn setup_script_functions(&self, vars: Arc<RwLock<HashMap<String, String>>>, evtq: Arc<Mutex<VecDeque<DerivedEvent>>>) -> Result<()> {
        let vars_to_set = Arc::clone(&vars);
        let vars_to_get = Arc::clone(&vars);
        self.lua.context::<_, rlua::Result<()>>(|lua_ctx| {
            let globals = lua_ctx.globals();
            // initialize SetVariable function
            let set_variable = lua_ctx.create_function(move |_, (k, v): (String, String)| {
                let mut m = vars_to_set.write().unwrap();
                m.insert(k, v);
                Ok(())
            })?;
            globals.set("SetVariable", set_variable)?;
            // initialize GetVariable function
            let get_variable = lua_ctx.create_function(move |_, k: String| {
                let m = vars_to_get.read().unwrap();
                if let Some(v) = m.get(&k) {
                    Ok(v.to_owned())
                } else {
                    Ok(String::new())
                }
            })?;
            globals.set("GetVariable", get_variable)?;
            {
                // initialize SwitchCodec function
                let evtq = evtq.clone();
                let switch_codec = lua_ctx.create_function(move |_, code: String| {
                    let new_code = match &code.to_lowercase()[..] {
                        "gbk" => Codec::Gb18030,
                        "utf8" | "utf-8" => Codec::Utf8,
                        "big5" => Codec::Big5,
                        _ => return Ok(()),
                    };
                    evtq.lock().unwrap().push_back(DerivedEvent::SwitchCodec(new_code));
                    Ok(())
                })?;
                globals.set("SwitchCodec", switch_codec)?;
            }
            {
                // initialize Send function
                let evtq = evtq.clone();
                let send = lua_ctx.create_function(move |_, mut s: String| {
                    eprintln!("Send function called");
                    if !s.ends_with('\n') {
                        s.push('\n');
                    }
                    evtq.lock().unwrap().push_back(DerivedEvent::StringToMud(s));
                    Ok(())
                })?;
                globals.set("Send", send)?;
            }
            {
                // initialize Note function
                let evtq = evtq.clone();
                let note = lua_ctx.create_function(move |_, s: String| {
                    eprintln!("Note function called");
                    let note_style = Style::default().fg(Color::LightBlue);
                    let sm = StyledLine{spans: vec![Span::styled(s.clone(), note_style)], orig: s, ended: true};
                    let mut sms = VecDeque::new();
                    sms.push_back(sm);
                    evtq.lock().unwrap().push_back(DerivedEvent::DisplayLines(sms));
                    Ok(())
                })?;
                globals.set("Note", note)?;
            }
            Ok(())
        })?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Plain(String),
    Regex(Regex),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Target {
    World,
    Script,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_set_variables() {
        let vars = Arc::new(RwLock::new(HashMap::new()));
        let evtq = Arc::new(Mutex::new(VecDeque::new()));
        let script = Script::new();
        script.setup_script_functions(vars.clone(), evtq).unwrap();
        script.exec("SetVariable('a', 'b')").unwrap();
        let r = {
            let vars_to_get = vars.read().unwrap();
            vars_to_get.get("a").cloned()
        };
        assert_eq!(Some("b".to_owned()), r);
    }

    #[test]
    fn test_script_non_existing_function() {
        let script = Script::new();
        assert!(script.exec("NonExistingFunc()").is_err());
    }

    struct FakeWriter;

    impl std::io::Write for FakeWriter {
        #[allow(unused_variables)]
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Ok(0)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

}