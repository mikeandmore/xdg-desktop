use regex::Regex;

use crate::desktop_parser::{DesktopFile, DesktopParserCallback};
use crate::dirs;
use core::{fmt, str};
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::fs::{read_dir, File, OpenOptions};
use std::io::Write;
use std::mem::swap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub struct MenuItemDetailEntry {
    pub exec: String,
    pub wmclass: String,
    pub is_terminal: bool,
    pub mimes: Vec<String>,
}

pub enum MenuItemDetail {
    Entry (MenuItemDetailEntry),
    Directory,
    Unknown,
}

impl MenuItemDetailEntry {
    fn guess_wmclass(&mut self) -> String {
	let args = self.exec.split(" ").collect::<Vec<&str>>();
	let cmd_prefix = "--command=";
	if args[0].ends_with("flatpak") {
	    for arg in &args[1..] {
		if arg.starts_with(cmd_prefix) {
		    return String::from(&arg[cmd_prefix.len()..]);
		}
	    }
	}

	return String::from(args[0].split("/").last().unwrap());
    }
    pub fn exec_with_filenames(&self, paths: &Vec<&PathBuf>) -> Vec<String> {
        let escape_path = |m: &str, p: &&PathBuf| -> String {
            let s = p.to_str().unwrap().replace('\'', "\\\'");
            if m == "%U" || m == "%u" {
                format!("\"file://{}\"", s)
            } else {
                format!("\"{}\"", s)
            }
        };
        let marker_regex = Regex::new("%[uUfF%]").unwrap();
        let mut result: Vec<String> = Vec::new();
        let mut next_path_id = 0;

        while next_path_id < paths.len() {
            let mut sstart: usize = 0;
            let mut fragments: Vec<String> = vec![];
            for m in marker_regex.find_iter(&self.exec) {
                fragments.push(self.exec[sstart..m.start()].to_string());
                sstart = m.end();

                if m.as_str() == "%U" || m.as_str() == "%F" {
                    fragments.push(paths.iter().map(|p| escape_path(m.as_str(), p)).fold(String::new(), |a, b| a + " " + b.as_str()));
                    next_path_id = paths.len();
                } else if m.as_str() == "%u" || m.as_str() == "%f" {
                    fragments.push(escape_path(m.as_str(), &paths[next_path_id]));
                    next_path_id += 1;
                }
            }
            result.push(fragments.join(""));
        }

        result
    }
}

pub struct MenuItem {
    pub name: String,
    pub icon: String,
    pub categories: String,
    pub basename: String,
    idx: usize,
    pub hidden: bool,
    pub detail: MenuItemDetail,
}

impl MenuItem {
    fn new() -> Self {
	MenuItem {
	    name: String::new(), icon: String::new(), categories: String::new(),
	    idx: 0, basename: String::new(), hidden: false, detail: MenuItemDetail::Unknown,
	}
    }
    fn root() -> Self {
	MenuItem {
	    name: String::from("FvwmApplications"), icon: String::from("_root"), categories: String::new(),
	    idx: 0, basename: String::from(""), hidden: true, detail: MenuItemDetail::Directory,
	}
    }

    fn other() -> Self {
	MenuItem {
	    name: String::from("Others"), icon: String::from("applications-other"), categories: String::new(),
	    idx: 1, basename: String::from("__other_apps"), hidden: false, detail: MenuItemDetail::Directory,
	}
    }

    pub fn detail_entry(&self) -> Option<&MenuItemDetailEntry> {
        if let MenuItemDetail::Entry(ent) = &self.detail {
            Some(ent)
        } else {
            None
        }
    }
}

pub struct Menu {
    pub item_idx: usize,
    pub children: Vec<usize>,
}

pub trait MenuPrinter {
    fn print(&mut self, item: &MenuItem);
    fn enter_menu(&mut self, item: &MenuItem);
    fn leave_menu(&mut self, item: &MenuItem);
}

impl Menu {
    fn new(item_idx: usize) -> Self {
	Menu {
	    item_idx, children: vec![],
	}
    }
    fn print(&self, index: &MenuIndex, printer: &mut impl MenuPrinter) {
	if self.children.is_empty() {
	    return;
	}

	let menu_ref = &index.items[self.item_idx];

	printer.print(menu_ref);

	printer.enter_menu(menu_ref);
	for idx in self.children.as_slice() {
	    let item = &index.items[*idx];
	    match item.detail {
		MenuItemDetail::Directory => {
		    let Some(submenu) = index.index.get(&item.basename) else {
			continue;
		    };
		    submenu.print(index, printer);
		},
		_ => printer.print(&item),
	    }
	}
	printer.leave_menu(menu_ref);
    }
}

struct MenuIndexDesktopParser {
    name_str: String,
    filename: String,

    current: MenuItem,
    current_key: String,
    in_action: bool,
}

impl DesktopParserCallback for MenuIndexDesktopParser {
    fn on_section(&mut self, name: &[u8]) -> bool {
	if name.starts_with(b"Desktop Action") {
	    self.in_action = true;
	} else if name.starts_with(b"Desktop Entry") {
	    self.current.detail = MenuItemDetail::Entry(MenuItemDetailEntry{ exec: String::new(), wmclass: String::new(), is_terminal: false, mimes: vec![] })
	} else {
            eprintln!("Unrecognized section {}", String::from_utf8_lossy(name));
            return false;
	}
        return true;
    }
    fn on_key(&mut self, key: &[u8]) -> bool {
	if !self.in_action {
	    self.current_key = decode(key);
        }

        true
    }
    fn on_value(&mut self, value: &[u8]) -> bool {
	if self.in_action {
	    return true;
	}

	if self.current_key == "Type" && value == b"Directory" {
	    self.current.detail = MenuItemDetail::Directory;
	} else if self.current_key == self.name_str {
	    self.current.name = decode(value);
	} else if self.current_key == "Icon" {
	    self.current.icon = decode(value);
	} else if self.current_key == "Categories" {
	    self.current.categories = decode(value);
	} else if self.current_key == "NoDisplay" {
	    self.current.hidden = value.to_ascii_lowercase() == b"true";
	} else if let MenuItemDetail::Entry(detail) = &mut self.current.detail {
	    if self.current_key == "Exec" {
		detail.exec = decode(value);
	    } else if self.current_key == "StartupWMClass" {
		detail.wmclass = decode(value);
	    } else if self.current_key == "Terminal" {
                detail.is_terminal = value.to_ascii_lowercase() == b"true";
            } else if self.current_key == "MimeType" {
                detail.mimes = String::from_utf8_lossy(value).split(';').map(|s| s.to_string()).collect();
            }
	}

        true
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum AssocType {
    Default, Add, Remove,
}

#[derive(Clone)]
pub struct Assoc {
    pub filename: String,
    pub mime: String,
    pub assoc_type: AssocType,
}

impl fmt::Display for AssocType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AssocType::Add => write!(f, "Add Associations"),
            AssocType::Remove => write!(f, "Removed Associations"),
            AssocType::Default => write!(f, "Default Applications"),
        }
    }
}

struct MenuIndexAssocParser {
    cur_mime: String,
    cur_assoc: AssocType,

    assocs: Vec<Assoc>,
}

impl DesktopParserCallback for MenuIndexAssocParser {
    fn on_section(&mut self, name: &[u8]) -> bool {
        if name.starts_with(b"Default Applications") {
            self.cur_assoc = AssocType::Default;
        } else if name.starts_with(b"Add Associations") {
            self.cur_assoc = AssocType::Add;
        } else if name.starts_with(b"Removed Associations") {
            self.cur_assoc = AssocType::Remove;
        } else {
            eprintln!("Unrecognized section {}", String::from_utf8_lossy(name));
            return false;
        }

        true
    }

    fn on_key(&mut self, key: &[u8]) -> bool {
        self.cur_mime = String::from_utf8_lossy(key).to_string();
        true
    }

    fn on_value(&mut self, value: &[u8]) -> bool {
        for s in value.to_vec().split(|ch| *ch == b';') {
            if s.len() == 0 {
                continue;
            }
            let Ok(filename) = str::from_utf8(s) else {
                continue;
            };
            self.assocs.push(Assoc { filename: filename.to_string(), mime: self.cur_mime.clone(), assoc_type: self.cur_assoc });
        }

        true
    }
}

pub struct MenuAssociation {
    pub default: Option<usize>,
    pub all: Vec<usize>,
}

pub struct MenuIndex {
    pub index: HashMap<String, Menu>,
    pub mime_assoc_index: HashMap<String, MenuAssociation>,
    pub items: Vec<MenuItem>,
    pub local_assocs: Vec<Assoc>,

    filename_index: HashMap<String, usize>,

    desk_parser: MenuIndexDesktopParser,
    assoc_parser: MenuIndexAssocParser,
}

fn decode(bytes: &[u8]) -> String { return String::from_utf8_lossy(bytes).into_owned(); }

impl MenuIndex {
    pub fn new_default() -> Self {
	MenuIndex::new(None)
    }

    pub fn new(locale: Option<String>) -> Self {
	let mut name_str = String::from("Name");
	if let Some(lc) = locale {
	    name_str += "[";
	    name_str += &lc;
	    name_str += "]";
	}
	let other_item = MenuItem::other();
        let desk_parser = MenuIndexDesktopParser {
            name_str,
	    filename: other_item.basename.clone(),
	    current: other_item,
	    current_key: String::new(),
	    in_action: false,
        };
        let assoc_parser = MenuIndexAssocParser {
            cur_mime: String::new(),
            cur_assoc: AssocType::Default,
            assocs: vec![],
        };
	return MenuIndex {
	    index: HashMap::from([(String::new(), Menu::new(0))]),
            mime_assoc_index: HashMap::new(),
	    items: vec![MenuItem::root()],
            local_assocs: Vec::new(),
            filename_index: HashMap::new(),
	    desk_parser,
            assoc_parser,
	}
    }

    fn desk_parser_reset(&mut self) -> bool {
	let mut current = MenuItem::new();
	swap(&mut current, &mut self.desk_parser.current);
	self.desk_parser.in_action = false;
	if !current.name.is_empty() {
	    current.basename = self.desk_parser.filename.clone();
	    current.idx = self.items.len();
	    if let MenuItemDetail::Directory = current.detail {
		self.index.insert(self.desk_parser.filename.clone(), Menu::new(current.idx));
	    } else if let MenuItemDetail::Entry(detail) = &mut current.detail {
		if detail.wmclass.is_empty() {
		    // Guess the wmclass
		    detail.wmclass = detail.guess_wmclass();
		}
	    }
	    self.items.push(current);

            return true;
	}
        return false;
    }
    fn assoc_parser_reset(&mut self) -> Vec<Assoc> {
        self.assoc_parser.cur_mime = String::new();
        let mut result: Vec<Assoc> = vec![];
        swap(&mut result, &mut self.assoc_parser.assocs);

        result
    }

    pub fn scan(&mut self) {
        let paths = dirs::xdg_data_dirs();
        self.scan_all(paths.iter().map(|s| Path::new(s)));
    }

    pub fn scan_all<'a, PathIterator>(&mut self, paths: PathIterator)
    where PathIterator: Iterator<Item = &'a Path> {
	self.desk_parser_reset();

	for p in paths {
	    if p.is_dir() {
		self.scan_prefix_path(p);
	    }
	}

	// Connect all items.
	for item in &self.items {
	    if item.idx == 0 {
		continue;
	    }

	    if item.categories.is_empty() {
		if let MenuItemDetail::Directory = item.detail {
		    self.index.get_mut("").unwrap().children.push(item.idx);
		    continue;
		}
	    }

	    let mut in_menu = false;
	    for key in item.categories.split(";") {
		if key == "" { continue; }
		if let Some(menu) = self.index.get_mut(key) {
		    menu.children.push(item.idx);
		    in_menu = true;
		} else {
		    // eprintln!("Cannot find category {} in {}", key, item.basename);
		}
	    }
	    if item.basename != "__other_apps" && !in_menu {
		// eprintln!("adding {} Others...", item.basename);
		self.index.get_mut("__other_apps").unwrap().children.push(item.idx);
	    }
	}

        // Build MIME associations.
        for i in 0..self.items.len() {
            let MenuItemDetail::Entry(ent) = &self.items[i].detail else {
                continue;
            };
            for mime in ent.mimes.iter() {
                if self.mime_assoc_index.get_mut(mime.as_str()).map(|assoc| { assoc.all.push(i); }).is_none() {
                    self.mime_assoc_index.insert(mime.clone(), MenuAssociation { default: None, all: vec![i] });
                }
            }
        }
    }

    fn scan_prefix_path(&mut self, p: &Path) {
	let app_dir = p.join("applications");
	let dir_dir = p.join("desktop-directories");
	for (p, ext) in [(app_dir, "desktop"), (dir_dir, "directory")] {
	    let Ok(dir) = read_dir(&p) else {
		continue;
	    };
	    for dirent in dir {
		let Ok(ent) = dirent else {
		    eprintln!("invalid dirent");
		    continue;
		};
		let path = ent.path();
		if !path.is_file() || !path.extension().is_some_and(|e| e == ext) {
		    // eprintln!("ignoring file {} expecting ext {}", &path.display(), ext);
		    continue;
		}
		let Some(filename) = path.file_name().unwrap().to_str() else {
		    eprintln!("cannot decode filename {}", &path.display());
		    continue;
		};

		self.desk_parser.filename = filename[..filename.len() - path.extension().unwrap().len() - 1].to_string();
		let Ok(file) = File::open(path.clone()) else {
		    eprintln!("Cannot open {}", path.to_str().unwrap());
		    continue;
		};
		let Ok(parser) = DesktopFile::new(file) else {
		    eprintln!("Cannot parse {}", path.to_str().unwrap());
		    continue;
		};

		// eprintln!("Parsing file {}", path.to_str().unwrap());
		parser.parse(&mut self.desk_parser);
		if self.desk_parser_reset() {
                    self.filename_index.insert(filename.to_string(), self.items.len() - 1);
                }
	    }
            if ext == "directory" {
                continue;
            }

            let Ok(mime_assoc_file) = File::open(p.join("mimeapps.list")) else {
                continue;
            };
            let Ok(assoc_parser) = DesktopFile::new(mime_assoc_file) else {
                continue;
            };
            assoc_parser.parse(&mut self.assoc_parser);
            let assocs = self.assoc_parser_reset();
            let local_dir = env::var("HOME").unwrap_or("/root".to_string()) + "/.local/share/applications";
            if p == OsString::from_str(local_dir.as_str()).unwrap() {
                self.local_assocs = assocs.clone();
            }
            for assoc in assocs {
                let Some(idx) = self.filename_index.get(&assoc.filename) else {
                    continue;
                };
                let MenuItemDetail::Entry(ent) = &mut self.items[*idx].detail else {
                    continue;
                };

                if assoc.assoc_type == AssocType::Add {
                    ent.mimes.push(assoc.mime);
                } else if assoc.assoc_type == AssocType::Remove {
                    if let Some(to_remove) = ent.mimes.iter().position(|m| *m == assoc.mime) {
                        ent.mimes.remove(to_remove);
                    }
                } else if assoc.assoc_type == AssocType::Default {
                    self.mime_assoc_index.insert(assoc.mime.clone(), MenuAssociation { default: Some(*idx), all: vec![] });
                }
            }
	}
    }

    pub fn print(&self, printer: &mut impl MenuPrinter) {
	self.index.get("").unwrap().print(self, printer);
    }

    pub fn change_default_assoc(&mut self, mime: &str, idx: usize) {
        let filename = self.items[idx].basename.clone() + ".desktop";
        let mut old_default: Option<usize> = None;
        if self.mime_assoc_index.get_mut(mime).map(|assoc| { old_default = std::mem::replace(&mut assoc.default, Some(idx)); }).is_none() {
            self.mime_assoc_index.insert(mime.to_string(), MenuAssociation { default: Some(idx), all: Vec::new() });
        }

        if old_default.is_none() {
            self.local_assocs.push(Assoc { filename, mime: mime.to_string(), assoc_type: AssocType::Default });
            return;
        }

        for assoc in self.local_assocs.iter_mut() {
            if assoc.assoc_type == AssocType::Default && assoc.filename == filename && assoc.mime.as_str() == mime {
                assoc.filename = filename;
                break;
            }
        }
    }

    pub fn write_default_assoc(&self) -> std::io::Result<()> {
        let mut file = OpenOptions::new().write(true).truncate(true).create(true).open(env::var("HOME").unwrap_or("/root".to_string()) + "/.local/share/applications/mimeapps.list")?;
        let mut cur_sec: Option<AssocType> = None;
        for assoc in &self.local_assocs {
            if cur_sec != Some(assoc.assoc_type) {
                file.write_fmt(format_args!("[{}]\n", assoc.assoc_type))?;
                cur_sec = Some(assoc.assoc_type);
            }
            file.write_fmt(format_args!("{}={}\n", &assoc.mime, &assoc.filename))?;
        }

        Ok(())
    }
}
