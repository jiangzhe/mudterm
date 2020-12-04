//! MUD eXtension Protocol
//!
//! A good introduction to how to implement it in MUSHClient:
//! http://www.gammon.com.au/forum/bbshowpost.php?bbsubject_id=222
use crate::ui::line::Line;
use crate::ui::style::Color;

pub fn supports() -> &'static str {
    "+head +body +afk +title +username +pass +samp +h +high +i +option +bold +xch_page +reset +strong +recommend_option +support +ul +em +send +send.href +send.hint +send.xch_cmd +send.xch_hint +send.prompt +p +hr +html +user +password +a +a.href +a.xch_cmd +a.xch_hint +underline +b +img +img.src +img.xch_mode +pre +li +ol +c +c.fore +c.back +font +font.color +font.back +font.fgcolor +font.bgcolor +u +mxp +mxp.off +version +br +v +var +italic"
}

pub enum State {
    Open,
    Secure,
    Locked,
    // Reset,
    // TempSecur,
    // LockOpen,
    // LockSecure,
    // LockLocked,
}

/// 定义MXP Tags
/// https://www.zuggsoft.com/zmud/mxp.htm
#[derive(Debug, Clone)]
pub enum MxpToken {
    Bold(bool),
    Italic(bool),
    Underline(bool),
    Strikeout(bool),
    // 前景色，背景色
    Color{
        fg: Color, 
        bg: Option<Color>,
    },
    High(bool),
    // 字体名，字体大小，前景色，背景色
    Font{
        face: String, 
        size: Option<u32>, 
        fg: Option<Color>, 
        bg: Option<Color>,
    },
    // 忽略其后的\n
    NoBr,
    // 段落，其中所有\n被忽略
    P(bool), 
    // 换行，MXP模式中不自动切换模式
    Br,
    // 软换行，客户端可以使用空格替代，单在换行模式下建议换行
    Sbr,
    // 代替空格
    Nbsp,
    A{
        href: String,
        hint: Option<String>,
        expire: Option<String>,
    },
    AEnd,
    Send{
        href: Option<String>,
        hint: Option<String>,
        prompt: bool,
        text: Option<String>,
    },
    SendEnd,
    Expire(String),
    // 向客户端查询MXP版本
    Version,
    // 向客户端查询支持的标签列表
    Support,
    // 所有非上述标签全部归为Text
    Text(String),
}

