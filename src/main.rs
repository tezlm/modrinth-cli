use std::env;
use std::io::{Read, Write};
use colored::*;
use serde::{Serialize, Deserialize};
use attohttpc::get;
use inquire::{Text, Confirm};
use semver::{Version,VersionReq};
use progress_bar::{color::{Style,Color},progress_bar::ProgressBar};

#[derive(Serialize, Deserialize, Debug)]
struct Options {
    version: String,
    mods: Vec<OptionMod>,
}

#[derive(Serialize, Deserialize, Debug)]
struct OptionMod {
    id: String,
    url: String,
    filename: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct MinecraftMods {
    hits: Vec<MinecraftMod>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MinecraftMod {
    #[serde(rename = "mod_id")]
    id: String,

    title: String,
    author: String,
    description: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ModVersion {
    #[serde(rename = "game_versions")]
    versions: Vec<String>,
    
    loaders: Vec<String>,
    files: Vec<ModFile>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ModFile {
    url: String,
    filename: String,
}

enum ModState { Installed(String), Uninstalled(String) }

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

fn search_mods(query: &String) -> Result<(String, MinecraftMods), std::io::Error> {
    let query = if query.is_empty() {
        Text::new("query").prompt().unwrap()
    } else {
        query.to_string()
    };
    let mut mods: MinecraftMods = get("https://api.modrinth.com/api/v1/mod")
        .param("query", &query)
        .send()?
        .json()?;
    for m in &mut mods.hits {
        m.id = m.id.replace("local-", "");
    }
    Ok((query, mods))
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

fn find_correct_version(id: &String, target: &Version) -> Result<ModFile, std::io::Error> {
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

fn install(id: &String, target: &Version) -> Result<OptionMod, std::io::Error> {
    let bullseye = find_correct_version(&id, &target).expect("couldnt find mod");
    let ModFile { url, filename } = bullseye;

    let res = get(&url).send()?;
    let (_, headers, mut body) = res.split();
    let len: u32 = headers
        .get("Content-Length")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
        .parse()
        .unwrap();
    
    let mut bar = ProgressBar::new(len as usize);
    let mut progress = 0;
    let mut buffer = [0u8; 0x4000];
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&filename)?; 
    
    bar.set_action("downloading", Color::Cyan, Style::Normal);

    loop {
        let size = body.read(&mut buffer)?;
        if size == 0 { break }
        file.write_all(&buffer).unwrap();
        progress += size;
        bar.set_progression(progress);
    }

    bar.set_action("downloaded", Color::Green, Style::Normal);
    bar.finalize();

    Ok(OptionMod {
        id: id.to_string(),
        filename: filename.to_owned(),
        url: url.to_owned(),
    })
}

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
            let (_, mods) = search_mods(&query).unwrap();
            for i in mods.hits {
                print_mod(&i);
            };
        }
        "install" | "i" => {
            let (query, mods) = search_mods(&query).unwrap();
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
