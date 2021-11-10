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
        Ok(str) => serde_json::from_str(&str).expect("invalid json in mods.json!"),
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
                Err(_) => true,
            });
        if found {
            return Ok(version.files.get(0).unwrap().to_owned());
        }
    }
 
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, "cant find correct version"))
}

fn install<T: std::io::Write>(url: &String, filename: &String, mut bar: ProgressBar<T>) -> Result<(), std::io::Error> {
    let res = get(&url).send()?;
    let (_, headers, mut body) = res.split();
    let len = headers
        .get("Content-Length")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
        .parse()
        .unwrap();

    let mut buffer = [0u8; 0x4000];
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .append(false)
        .create(true)
        .open(&filename)?; 
    
    bar.total = len;
    bar.set_units(Units::Bytes);
    bar.show_tick = false;
    bar.show_percent = false;
    bar.show_time_left = false;
    bar.show_speed = false;
    bar.message(&format!("{} {} ", "downloading".cyan().bold(), &filename));

    loop {
        let size = body.read(&mut buffer)?;
        if size == 0 { break }
        file.write_all(&buffer[0..size]).unwrap();
        bar.add(size as u64);
    }

    bar.finish_println(&format!("{} {} ", "downloaded".green().bold(), &filename));

    Ok(())
}

fn install_single(id: &String, target: &Version) -> Result<OptionMod, std::io::Error> {
    let bullseye = find_correct_version(&id, &target)?;
    let ModFile { url, filename } = bullseye;
    let bar = ProgressBar::new(0);

    install(&url, &filename, bar)?;
    
    Ok(OptionMod {
        id: id.to_string(),
        filename: filename.to_owned(),
        url: url.to_owned(),
    })
}

fn install_pack(mods: &Vec<OptionMod>) -> Result<(), std::io::Error> {
    println!("{}", "downloading mods".cyan().bold());

    for m in mods {
        if std::fs::metadata(&m.filename).is_ok() { continue }
        let bar = ProgressBar::new(0);
        install(&m.url, &m.filename, bar)?;
    }
    
    println!("\r{}{}", crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine), "done!".green().bold());

    Ok(())
}

fn already_installed(id: &String, options: &Options) -> bool {
    match options.mods.iter().find(|h| h.id.eq(id)) { 
        Some(found) => {
            if std::fs::metadata(&found.filename).is_ok() {
                true
            } else {
                false
            }
        },
        None => false,
    }
}

fn find_mod(query: &str, mods: &MinecraftMods, options: &Options) -> Option<ModState> {
    if mods.hits.len() == 1 {
        let id = mods.hits[0].id.to_owned();
        if already_installed(&id, &options) {
            return Some(ModState::Installed(id));
        } else {
            return Some(ModState::Uninstalled(id));
        }
    }

    for (i, m) in mods.hits.iter().enumerate() {
        if m.title.to_lowercase() == query {
            if already_installed(&m.id, &options) {
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
            println!("    {}:    {}", "pack, p".blue(), "install from mods.json");
        },
        "search" | "s" => {
            let mods = search_mods(&query)?;
            for i in mods.hits {
                print_mod(&i);
            };
        },
        "install" | "i" => {
            let query = if query.is_empty() { Text::new("query").prompt().unwrap() } else { query };
            let mods = search_mods(&query)?;
            match find_mod(&query, &mods, &options) {
                Some(ModState::Uninstalled(m)) => {
                    let version = Version::parse(&options.version).unwrap();
                    match install_single(&m, &version) {
                        Ok(m) => options.mods.push(m),
                        Err(why) => println!("{} {}", "error:".bold().red(), why),
                    };
                },
                Some(ModState::Installed(_)) => {
                    println!("already installed!");
                },
                None => {
                    println!("{} no mods", "error:".bold().red());
                },
            };
        },
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
        },
        "pack" | "p" => {
            if options.mods.len() == 0 {
                println!("no mods in pack!");
            } else {
                install_pack(&options.mods)?;
            }
        },
        "update" | "u" => {
            let mut outdated = Vec::new();
            let version = Version::parse(&options.version).unwrap();
            for m in &mut options.mods {
                let ModFile { url, .. } = find_correct_version(&m.id, &version).unwrap();
                if url != m.url { outdated.push(m) }
            }
            for m in &mut outdated {
                std::fs::remove_file(&m.filename).unwrap_or_default();
                let OptionMod { filename, url, ..} = install_single(&m.id, &version)?;
                m.filename = filename;
                m.url = url;
            }
        },
        _ => {
            println!("{} {} is not a command", "error:".bold().red(), subcommand);
            return Ok(());
        },
    }
    
    std::fs::write("mods.json", serde_json::to_string(&options)?)?;
    Ok(())
}

