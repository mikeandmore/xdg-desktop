use xdg_desktop::dirs::xdg_data_dirs;
use xdg_desktop::icon::IconCollection;
use xdg_desktop::menu::{MenuPrinter, MenuItem, MenuItemDetail, MenuIndex};
use std::path::PathBuf;
use std::{env, path::Path, process::Command, fs};
use std::io;

struct FvwmMenuPrinter<'a> {
    level: usize,
    icon_col: IconCollection,
    desire_icon_size: usize,
    menu_index: &'a MenuIndex,

    menu_stack: Vec<String>,
}

impl<'a> FvwmMenuPrinter<'a> {
    fn new<'b, PathIterator>(icon_theme: String, paths: PathIterator, desire_icon_size: usize, menu_index: &'a MenuIndex) -> Self
    where PathIterator: Iterator<Item = &'b Path> {
	let pathname = format!("{}/.fvwm/icons/{}", env::var("HOME").unwrap(), desire_icon_size);
	let local_icon_path = Path::new(&pathname);
	if !local_icon_path.is_dir() {
	    let _ = fs::create_dir(local_icon_path);
	}

	let mut icon_col = IconCollection::new();
	icon_col.scan_with_theme(vec![&icon_theme, "hicolor"], paths);

	Self {
	    level: 0, icon_col, desire_icon_size, menu_index, menu_stack: vec!(),
	}
    }

    fn ensure_all_icons(&self) {
	for item in &self.menu_index.items {
	    if let Err(err) = self.ensure_icon(&item.icon) {
		eprintln!("Error when converting icons {} {}", &item.icon, err.to_string());
	    }
	}
    }

    fn ensure_icon(&self, name: &str) -> Result<(), io::Error> {
        let icon_path: PathBuf;
        let icon_name: String;

        if name.starts_with('/') {
            // Always convert because we don't really trust this file.
            icon_path = PathBuf::from(name);
            icon_name = String::from(icon_path.file_stem().unwrap().to_str().unwrap());
        } else {
	    let Some((icon_desc, match_icon_path)) = self.icon_col.find_icon(name, self.desire_icon_size) else {
	        return Ok(());
	    };
	    if icon_desc.icon_size() == self.desire_icon_size {
	        return Ok(());
	    }
            if match_icon_path.extension().unwrap() == "svg" {
                return Ok(());
            }
            icon_path = match_icon_path;
            icon_name = String::from(name);
        }

	// Call imagemagick convert to scale the image.
	let output_filename = format!("{}/.fvwm/icons/{}/{}.png", env::var("HOME").unwrap(), self.desire_icon_size, icon_name);

	let src_mod = fs::metadata(&icon_path)?.modified()?;
	if let Ok(dst_md) = fs::metadata(&output_filename) {
	    if let Ok(dst_mod) = dst_md.modified() {
		if dst_mod > src_mod {
		    return Ok(());
		}
	    }
	}

	let result = Command::new("convert")
	    .arg("-resize").arg(format!("{}x{}", self.desire_icon_size, self.desire_icon_size))
	    .arg(icon_path.to_str().unwrap())
	    .arg(&output_filename)
	    .spawn();
	if !result?.wait()?.success() {
	    Err(io::Error::new(io::ErrorKind::Other, "convert failed"))
	} else {
	    Ok(())
	}
    }

    fn resolve_icon(&self, name: &str) -> Option<String> {
        if name.starts_with('/') {
            // They are always converted.
            return Some(format!("{}/.fvwm/icons/{}/{}.png", env::var("HOME").unwrap(), self.desire_icon_size,
                                PathBuf::from(name).file_stem().unwrap().to_str().unwrap()));
        }

	let Some((icon_desc, icon_path)) = self.icon_col.find_icon(name, self.desire_icon_size) else {
	    return None;
	};

	if icon_path.extension().unwrap() == "svg" {
	    return Some(format!("{}:{}x{}", icon_path.to_str().unwrap(), self.desire_icon_size, self.desire_icon_size));
	}

	if icon_desc.icon_size() == self.desire_icon_size {
	    return Some(String::from(icon_path.to_str().unwrap()));
	}

	return Some(format!("{}/.fvwm/icons/{}/{}.png", env::var("HOME").unwrap(), self.desire_icon_size, &name));
    }

    fn print_wmclass_icons(&self) {
	for item in &self.menu_index.items {
	    let MenuItemDetail::Entry(detail) = &item.detail else {
		continue;
	    };
	    let Some(resolved_icon) = self.resolve_icon(&item.icon) else {
		continue;
	    };
	    println!("Style \"{}\" MiniIcon \"{}\"", detail.wmclass, resolved_icon);
	}
    }

    fn escape(&self, str: &str) -> String {
	str.replace("&", "&&")
    }
}

impl<'a> MenuPrinter for FvwmMenuPrinter<'a> {
    fn print(&mut self, item: &MenuItem) {
	if !item.hidden {
	    let mut frag = format!("+ \"{}{}\" ", self.escape(&item.name),
				   match self.resolve_icon(&item.icon) {
				       Some(icon) => format!("%{}%", icon),
				       None => String::new()
				   });

	    if let MenuItemDetail::Entry(detail) = &item.detail {
		frag.push_str(&format!("Exec exec {} {}\n", if detail.is_terminal { "xterm -e" } else { "" }, detail.exec));
	    } else if let MenuItemDetail::Directory = item.detail {
		frag.push_str(&format!("Popup \"{}\"\n", item.name));
	    }
	    self.menu_stack.last_mut().unwrap().push_str(&frag);
	}
    }

    fn enter_menu(&mut self, item: &MenuItem) {
	self.level += 1;
	let name = &item.name;
	self.menu_stack.push(format!("Destroymenu \"{}\"\nAddToMenu \"{}\" \"{}\" Title\n", name, name, name));
    }

    fn leave_menu(&mut self, _item: &MenuItem) {
	println!("{}\n", self.menu_stack.pop().unwrap());
	self.level -= 1;
    }
}

fn main() {
    let icon_theme = env::args().nth(1).unwrap().to_string();
    let mut index = MenuIndex::new_default();

    index.scan();
    let paths = xdg_data_dirs();
    let mut printer = FvwmMenuPrinter::new(icon_theme, paths.iter().map(|s| Path::new(s)), 64, &index);
    printer.ensure_all_icons();

    index.print(&mut printer);
    printer.print_wmclass_icons();
}
