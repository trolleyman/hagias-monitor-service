use rocket::figment::value::magic::RelativePathBuf;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub layouts_path: RelativePathBuf,
}
