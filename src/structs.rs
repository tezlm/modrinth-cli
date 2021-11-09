use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Options {
    pub version: String,
    pub mods: Vec<OptionMod>,
}

#[derive(Serialize, Deserialize)]
pub struct OptionMod {
    pub id: String,
    pub url: String,
    pub filename: String,
}

#[derive(Serialize, Deserialize)]
pub struct MinecraftMods {
    pub hits: Vec<MinecraftMod>,
}

#[derive(Serialize, Deserialize)]
pub struct MinecraftMod {
    #[serde(rename = "mod_id")]
    pub id: String,

    pub title: String,
    pub author: String,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct ModVersion {
    #[serde(rename = "game_versions")]
    pub versions: Vec<String>,
    
    pub loaders: Vec<String>,
    pub files: Vec<ModFile>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ModFile {
    pub url: String,
    pub filename: String,
}

pub enum ModState { Installed(String), Uninstalled(String) }

