use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Options {
    pub version: String,
    pub mods: Vec<OptionMod>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OptionMod {
    pub id: String,
    pub url: String,
    pub filename: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MinecraftMods {
    pub hits: Vec<MinecraftMod>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MinecraftMod {
    #[serde(rename = "mod_id")]
    pub id: String,

    pub title: String,
    pub author: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ModVersion {
    pub game_versions: Vec<String>,
    pub version_number: String,
    pub version_type: String,
    pub loaders: Vec<String>,
    pub files: Vec<ModFile>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModFile {
    pub url: String,
    pub filename: String,
}

pub enum ModState { Installed(String), Uninstalled(String) }

