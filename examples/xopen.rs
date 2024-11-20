use std::{collections::BTreeMap, env, io::stdin, iter, path::{Path, PathBuf}, process::Command};
use glob::Pattern;
use xdg_desktop::{menu::MenuIndex, mime_glob::mime_glob_foreach};

fn show_usage() {
    println!("{} [-s -u] file1 [file2 file3 ...]\n\n", env::args().nth(0).unwrap());
    println!(" -s: Select which app to open.\n");
    println!(" -u: Save the select app as the default when using with -s.\n");
}

fn main() {
    let mut select_app = false;
    let mut save_selection = false;
    let paths: Vec<PathBuf> = env::args().skip(1).filter_map(|pstr| {
        if pstr == "-s" {
            select_app = true;
            return None;
        } else if pstr == "-u" {
            save_selection = true;
            return None;
        }
        let path = Path::new(&pstr);
        let pathbuf = if path.is_symlink() {
            let Ok(pbuf) = path.read_link() else {
                eprintln!("Cannot read link {}", &pstr);
                return None;
            };
            pbuf
        } else {
            path.to_path_buf()
        };
        let path = Path::new(&pathbuf);
        if !path.exists() || !path.is_file() {
            eprintln!("Path {} does not exist", path.display());
            return None;
        }

        Some(pathbuf)
    }).collect();

    if paths.is_empty() {
        show_usage();
        return;
    }

    let mut mimes: Vec<String> = Vec::with_capacity(paths.len());
    let mut nr_matches = 0;
    mimes.extend(iter::repeat(String::new()).take(paths.len()));
    mime_glob_foreach(|_, m, pattern| {
        let ptn = Pattern::new(pattern).unwrap();

        for i in 0..paths.len() {
            if !mimes[i].is_empty() {
                continue;
            }

            let filename = paths[i].file_name().unwrap().to_str().unwrap();
            if ptn.matches(filename) {
                mimes[i] = m.clone();
                nr_matches += 1;
            }
        }

        nr_matches < paths.len()
    }).expect("Cannot find mime type for file");

    let mut index = MenuIndex::new_default();
    index.scan();

    let mut assoc_map: BTreeMap<usize, Vec<&PathBuf>> = BTreeMap::new();

    for i in 0..mimes.len() {
        let mime = mimes[i].as_str();
        if mime.is_empty() {
            println!("Cannot find MIME type for {}", &paths[i].display());
            continue;
        }
        let Some(assoc) = index.mime_assoc_index.get(mime) else {
            println!("Cannot find any associate app for {}", &paths[i].display());
            continue;
        };
        let idx;
        if !select_app && assoc.default.is_some() {
            let default_idx = assoc.default.unwrap();
            println!("Using Default: {}", &index.items[default_idx].name);
            idx = default_idx;
        } else {
            println!("No default app for {}. Select from the following apps:", mime);
            for j in 0..assoc.all.len() {
                println!("{}. {}", j, &index.items[assoc.all[j]].name);
            }
            let mut user_input = String::new();
            if stdin().read_line(&mut user_input).is_err() {
                return;
            }
            let Ok(sel) = user_input.trim().parse::<usize>() else {
                println!("Invalid selection");
                return;
            };
            if sel >= assoc.all.len() {
                println!("Invalid selection {}", sel);
                return;
            }
            idx = assoc.all[sel];
            if save_selection {
                index.change_default_assoc(mime, idx);
            }
        }
        if assoc_map.get_mut(&idx).map(|v| {v.push(&paths[i]);}).is_none() {
            assoc_map.insert(idx, vec![&paths[i]]);
        }
    }

    let cmds = assoc_map.iter().map(|(idx, v)| {
        let item = &index.items[*idx];
        item.detail_entry().unwrap().exec_with_filenames(v)
    }).flatten().collect::<Vec<String>>();

    println!("Will execute the following command(s):");
    for cmd in &cmds {
        println!("{}", cmd);
        let Ok(_) = Command::new("/bin/sh").arg("-c").arg(cmd).spawn() else {
            eprintln!("Fail to execute command");
            continue;
        };
    }
    if save_selection {
        index.write_default_assoc().unwrap();
    }

    return;
}
