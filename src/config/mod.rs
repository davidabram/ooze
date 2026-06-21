mod schema;
mod template;

use std::path::{Path, PathBuf};

use anyhow::Context;

pub use schema::OozeConfig;
pub use template::INIT_CONFIG_TEMPLATE;

pub const DEFAULT_CONFIG_NAME: &str = "ooze.toml";

pub fn load_config(path: Option<&Path>) -> anyhow::Result<(OozeConfig, Option<PathBuf>)> {
    let resolved = match path {
        Some(p) => Some(p.to_path_buf()),
        None => {
            let default = PathBuf::from(DEFAULT_CONFIG_NAME);
            if default.exists() { Some(default) } else { None }
        }
    };

    let Some(p) = resolved else {
        return Ok((OozeConfig::default(), None));
    };

    let text = std::fs::read_to_string(&p)
        .with_context(|| format!("reading config {}", p.display()))?;
    let config: OozeConfig = toml::from_str(&text)
        .with_context(|| format!("parsing config {}", p.display()))?;
    Ok((config, Some(p)))
}
