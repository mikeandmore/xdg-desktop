use crate::desktop_parser::{DesktopFile, DesktopParserCallback};
use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::mem::swap;
use std::path::Path;

pub struct MenuItemDetailEntry {
    pub exec: String,
    pub wmclass: String,
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
	    name: String::from("Others"), icon: String::from("others"), categories: String::new(),
	    idx: 1, basename: String::from("__other_apps"), hidden: false, detail: MenuItemDetail::Directory,
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

pub struct MenuIndex {
    pub index: HashMap<String, Menu>,
    pub items: Vec<MenuItem>,

    name_str: String,
    filename: String,

    current: MenuItem,
    current_key: String,
    in_action: bool,
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
	return MenuIndex {
	    index: HashMap::from([(String::new(), Menu::new(0))]),
	    items: vec![MenuItem::root()],
	    name_str,
	    filename: other_item.basename.clone(),
	    current: other_item,
	    current_key: String::new(),
	    in_action: false,
	}
    }
    fn reset(&mut self) {
	let mut current = MenuItem::new();
	swap(&mut current, &mut self.current);
	if !current.name.is_empty() {
	    current.basename = self.filename.clone();
	    current.idx = self.items.len();
	    if let MenuItemDetail::Directory = current.detail {
		self.index.insert(self.filename.clone(), Menu::new(current.idx));
	    } else if let MenuItemDetail::Entry(detail) = &mut current.detail {
		if detail.wmclass.is_empty() {
		    // Guess the wmclass
		    detail.wmclass = detail.guess_wmclass();
		}
	    }
	    self.items.push(current);
	}
	self.in_action = false;
    }

    pub fn scan_all(&mut self, paths: &Vec<&str>) {
	self.reset();

	for path_str in paths {
	    let p = Path::new(path_str);
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
		    eprintln!("Cannot find category {} in {}", key, item.basename);
		}
	    }
	    if item.basename != "__other_apps" && !in_menu {
		eprintln!("adding {} Others...", item.basename);
		self.index.get_mut("__other_apps").unwrap().children.push(item.idx);
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
		    eprintln!("ignoring path {} expecting ext {}", &path.display(), ext);
		    continue;
		}
		let Some(filename) = path.file_name().unwrap().to_str() else {
		    eprintln!("cannot decode filename {}", &path.display());
		    continue;
		};

		self.filename = filename[..filename.len() - path.extension().unwrap().len() - 1].to_string();
		let Ok(file) = File::open(path.clone()) else {
		    eprintln!("Cannot open {}", path.to_str().unwrap());
		    continue;
		};
		let Ok(parser) = DesktopFile::new(file) else {
		    eprintln!("Cannot parse {}", path.to_str().unwrap());
		    continue;
		};
		
		eprintln!("Parsing file {}", path.to_str().unwrap());
		parser.parse(self);
		self.reset();
	    }
	}
    }

    pub fn print(&self, printer: &mut impl MenuPrinter) {
	self.index.get("").unwrap().print(self, printer);
    }

}

impl DesktopParserCallback for MenuIndex {
    fn on_section(&mut self, name: &[u8]) {
	if name.starts_with(b"Desktop Action") {
	    self.in_action = true;
	} else if name.starts_with(b"Desktop Entry") {
	    self.current.detail = MenuItemDetail::Entry(MenuItemDetailEntry{ exec: String::new(), wmclass: String::new() })
	} else {
	    self.reset();
	}
    }
    fn on_key(&mut self, key: &[u8]) {
	if self.in_action {
	    return;
	}
	self.current_key = decode(key);
    }
    fn on_value(&mut self, value: &[u8]) {
	if self.in_action {
	    return;
	}

	if self.current_key == "Type" && value == b"Directory" {
	    self.current.detail = MenuItemDetail::Directory;
	    return;
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
	    }
	}
    }
}
