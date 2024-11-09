use std::{collections::HashMap, fs::File};
use std::io::Result;

use memmap::MmapOptions;
use regex::Regex;


struct MIMEGlobItem {
    mime: String,
    reg: Regex,
}

fn parse_mime_glob<'a, Callback>(slice: &'a [u8], mut callback: Callback) where Callback: FnMut(&'a [u8], &'a [u8]) -> () {
    let mut line_start = 0;
    loop {
        if line_start == slice.len() {
            break;
        }
        if slice[line_start] == b'#' {
            if let Some(line_end) = slice[line_start..].iter().position(|ch| *ch == b'\n') {
                line_start += line_end + 1;
                continue;
            } else {
                break;
            }
        }
        if let Some(colon_pos) = slice[line_start..].iter().position(|ch| *ch == b':') {
            if line_start + colon_pos + 1 == slice.len() {
                break;
            }
            if let Some(line_end) = slice[line_start + colon_pos + 1..].iter().position(|ch| *ch == b'\n') {
                let mime = &slice[line_start..line_start + colon_pos];
                let reg = &slice[line_start + colon_pos + 1..line_start + colon_pos + 1 + line_end];
                callback(mime, reg);

                line_start += colon_pos + line_end + 2;
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

pub struct MIMEGlobIndex {
    glob_regs: Vec<MIMEGlobItem>,
    glob_suffix_index: HashMap<String, String>,
}

impl MIMEGlobIndex {
    pub fn new() -> Result<Self> {
        let mut glob_regs: Vec<MIMEGlobItem> = vec![];
        let mut glob_suffix_index: HashMap<String, String> = HashMap::new();

        let file = File::open("/usr/share/mime/globs")?;
        let region = unsafe { MmapOptions::new().map(&file)? };

        parse_mime_glob(region.iter().as_slice(), |mime, reg| {
            // eprintln!("mime {} pattern {}", OsStr::from_bytes(mime).to_str().unwrap(), OsStr::from_bytes(reg).to_str().unwrap());
            if reg[0] == b'*' {
                glob_suffix_index.insert(
                    String::from_utf8(reg[1..].to_vec()).unwrap(),
                    String::from_utf8(mime.to_vec()).unwrap());
            } else {
                let reg_str = String::from_utf8(reg.to_vec()).unwrap();
                glob_regs.push(MIMEGlobItem {
                    mime: String::from_utf8(mime.to_vec()).unwrap(),
                    reg: Regex::new(&reg_str).unwrap(),
                });
            }
        });

        Ok(Self {
            glob_regs, glob_suffix_index,
        })
    }

    fn match_filename_suffix(&self, filename: &str) -> Option<&str> {
        if let Some(extpos) = filename.rfind('.') {
            let extosstr = &filename[extpos..];
            if let Some(mime) = self.glob_suffix_index.get(extosstr) {
                return Some(mime);
            }
        }

        None
    }

    fn match_filename_regex(&self, filename: &str) -> Option<&str> {
        for glob_item in &self.glob_regs {
            if glob_item.reg.is_match(filename) {
                return Some(glob_item.mime.as_str());
            }
        }

        None
    }

    pub fn match_filename(&self, filename: &str) -> Option<&str> {
        self.match_filename_suffix(filename).or_else(|| self.match_filename_regex(filename))
    }

}
