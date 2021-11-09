pub mod structs;
use crate::structs::*;
use std::env;
use std::io::{Read, Write};
use pbr::{ProgressBar, Units};
use colored::*;
use inquire::{Text, Confirm};
use attohttpc::get;
use semver::{Version,VersionReq};

fn parse_config() -> Options {
    match std::fs::read_to_string("./mods.json") {
        Ok(str) => serde_json::from_str(&str).unwrap(),
        Err(_) => {
            let version = Text::new("(exact) minecraft version")
                .with_validator(&|str| match Version::parse(str) {
                    Ok(_) => Ok(()),
                    Err(_) => Err("i said *exact* version".into()),
                })
                .prompt()
                .unwrap();
            Options { version, mods: Vec::new() }
        }
    }
}

fn get_command() -> (String, String) {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() == 0 { return ("help".into(), "".into()) };
    (args[0].to_owned(), args[1..].join(" "))
}

fn print_mod(mcmod: &MinecraftMod) {
    println!("{} {} - {} - {}",
        "=>".bright_black(),
        mcmod.title.bright_blue(),
        mcmod.author.blue(),
        mcmod.id.bright_black(),
    );
    println!("{}", mcmod.description);
    println!("");
}

pub fn search_mods(query: &String) -> Result<MinecraftMods, std::io::Error> {
    let mut mods: MinecraftMods = get("https://api.modrinth.com/api/v1/mod")
        .param("query", &query)
        .send()?
        .json()?;
    for m in &mut mods.hits {
        m.id = m.id.replace("local-", "");
    }
    Ok(mods)
}

pub fn find_correct_version(id: &String, target: &Version) -> Result<ModFile, std::io::Error> {
    let url = format!("https://api.modrinth.com/api/v1/mod/{}/version", id);
    let versions: Vec<ModVersion> = get(url)
        .send()?
        .json()?;

    for version in versions {
        if !version.loaders.contains(&"fabric".into()) {
            continue
        }
        let found = version
            .versions
            .iter()
            .any(|ver| match VersionReq::parse(ver) {
                Ok(ver) => ver.matches(&target),
                Err(_) => false,
            });
        if found {
            return Ok(version.files.get(0).unwrap().to_owned());
        }
    }
 
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, "cant find mod"))
}

pub fn download(url: &String) -> Result<(u64, attohttpc::ResponseReader), std::io::Error> {
    let res = get(&url).send()?;
    let (_, headers, body) = res.split();
    let len = headers
        .get("Content-Length")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
        .parse()
        .unwrap();
    
    Ok((len, body))
}

fn install(id: &String, target: &Version) -> Result<OptionMod, std::io::Error> {
    let bullseye = find_correct_version(&id, &target).expect("couldnt find mod");
    let ModFile { url, filename } = bullseye;

    let (len, mut body) = download(&url).unwrap();
    let mut bar = ProgressBar::new(len);
    let mut buffer = [0u8; 0x4000];
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .append(false)
        .create(true)
        .open(&filename)?; 
    
    bar.set_units(Units::Bytes);
    bar.show_tick = false;
    bar.show_percent = false;
    bar.show_time_left = true;
    bar.show_speed = false;
    bar.message(&format!("{} {} ", "downloading".cyan().bold(), &filename));

    loop {
        let size = body.read(&mut buffer)?;
        if size == 0 { break }
        file.write_all(&buffer[0..size]).unwrap();
        bar.add(size as u64);
    }

    bar.finish_print(&format!("{} {} ", "downloaded".green().bold(), &filename));

    Ok(OptionMod {
        id: id.to_string(),
        filename: filename.to_owned(),
        url: url.to_owned(),
    })
}

//fn install_pack(mods: &Vec<OptionMod>) -> Result<(), std::io::Error> {
//    let mut bar = ProgressBar::new(mods.len());
//    bar.set_action("downloading", Color::Cyan, Style::Normal);
//    
//    for m in mods {
//        let (len, mut body) = download(&m.url).unwrap();
//        let mut progress = 0;
//        let mut buffer = [0u8; 0x4000];
//        let mut file = std::fs::OpenOptions::new()
//            .write(true)
//            .create(true)
//            .open(&m.filename)?; 
//
//        loop {
//            let size = body.read(&mut buffer)?;
//            if size == 0 { break }
//            file.write_all(&buffer).unwrap();
//            progress += size;
//            bar.set_progression(progress);
//        }
//
//        bar.inc();
//    }
//
//    bar.set_action("downloaded", Color::Green, Style::Normal);
//    bar.finalize();
//    Ok(())
//}

fn find_mod(query: &str, mods: &MinecraftMods, options: &Options) -> Option<ModState> {
    if mods.hits.len() == 1 {
        let id = mods.hits[0].id.to_owned();
        if options.mods.iter().any(|h| h.id == id) {
            return Some(ModState::Installed(id));
        } else {
            return Some(ModState::Uninstalled(id));
        }
    }

    for (i, m) in mods.hits.iter().enumerate() {
        if m.title.to_lowercase() == query {
            if options.mods.iter().any(|h| h.id == m.id) {
                return Some(ModState::Installed(m.id.to_owned()));
            } else {
                return Some(ModState::Uninstalled(m.id.to_owned()));
            }
        }

        let mut conf = String::from(if i == 0 { "install " } else { "what about " });
        conf.push_str(&m.title);
        conf.push('?');
        if Confirm::new(&conf).prompt().unwrap() {
            return Some(ModState::Uninstalled(m.id.to_owned()));
        }
    }

    None
}

fn main() -> Result<(), std::io::Error> {
    let (subcommand, query) = get_command();
    let mut options = parse_config();
    match subcommand.as_str() {
        "--help" | "help" => {
            let border = "===".bright_black();
            println!("{} {} {}", border, "modrinth cli".bright_blue(), border);
            println!("download/update your mods!");
            println!("subcommands");
            println!("    {}:       {}", "help".blue(), "show this help");
            println!("    {}:  {}", "search, s".blue(), "search for a mod");
            println!("    {}: {}", "install, i".blue(), "install a mod");
            println!("    {}: {}", "remove, rm".blue(), "remove a mod");
            println!("    {}:  {}", "update, u".blue(), "update all mods");
        }
        "search" | "s" => {
            let mods = search_mods(&query).unwrap();
            for i in mods.hits {
                print_mod(&i);
            };
        }
        "install" | "i" => {
            let query = if query.is_empty() { Text::new("query").prompt().unwrap() } else { query };
            let mods = search_mods(&query).unwrap();
            match find_mod(&query, &mods, &options) {
                Some(ModState::Uninstalled(m)) => {
                    let version = Version::parse(&options.version).unwrap();
                    options.mods.push(install(&m, &version)?)
                },
                Some(ModState::Installed(_)) => {
                    println!("already installed!");
                },
                None => {
                    println!("{} no mods", "error:".bold().red());
                },
            };
        }
        "remove" | "rm" => {
            let query = query.to_lowercase();
            match options.mods.iter().position(|m| m.filename.to_lowercase().contains(&query)) {
                Some(i) => {
                    let file = &options.mods.get(i).unwrap().filename;
                    if std::fs::remove_file(&file).is_ok() {
                        println!("adios, {}", &file);
                    } else {
                        println!("{} cant find the file, removed anyway", "error:".bold().red());
                    }
                    options.mods.remove(i);
                },
                None => {
                    println!("{} cant find mod", "error:".bold().red());
                },
            };
        }
        "update" | "u" => {
            let mut outdated = Vec::new();
            let version = Version::parse(&options.version).unwrap();
            for m in &mut options.mods {
                let ModFile { url, .. } = find_correct_version(&m.id, &version).unwrap();
                if url != m.url { outdated.push(m) }
            }
            for m in &mut outdated {
                std::fs::remove_file(&m.filename).unwrap_or_default();
                let OptionMod { filename, url, ..} = install(&m.id, &version)?;
                m.filename = filename;
                m.url = url;
            }
        }
        _ => {
            eprintln!("{} {} is not a command", "error:".bold().red(), subcommand);
            return Ok(());
        }
    }
    std::fs::write("mods.json", serde_json::to_string(&options)?)?;
    Ok(())
}

