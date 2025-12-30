use std::sync::{LazyLock, RwLock};
use std::path::PathBuf;

use which::which_global;

//------------------------------------------------------------------------------
// STRUCT: Paths
//------------------------------------------------------------------------------
pub struct Paths;

impl Paths {
    //---------------------------------------
    // Paru path function
    //---------------------------------------
    pub fn paru() -> &'static which::Result<PathBuf> {
        static PARU_PATH: LazyLock<which::Result<PathBuf>> = LazyLock::new(|| {
            which_global("paru")
        });

        &PARU_PATH
    }

    //---------------------------------------
    // Paccat path function
    //---------------------------------------
    pub fn paccat() -> &'static which::Result<PathBuf> {
        static PACCAT_PATH: LazyLock<which::Result<PathBuf>> = LazyLock::new(|| {
            which_global("paccat")
        });

        &PACCAT_PATH
    }

    //---------------------------------------
    // Meld path function
    //---------------------------------------
    pub fn meld() -> &'static which::Result<PathBuf> {
        static MELD_PATH: LazyLock<which::Result<PathBuf>> = LazyLock::new(|| {
            which_global("meld")
        });

        &MELD_PATH
    }
}

//------------------------------------------------------------------------------
// STRUCT: Pacman
//------------------------------------------------------------------------------
pub struct Pacman;

impl Pacman {
    //---------------------------------------
    // Config function
    //---------------------------------------
    pub fn config() -> &'static pacmanconf::Config {
        static PACMAN_CONFIG: LazyLock<pacmanconf::Config> = LazyLock::new(|| {
            pacmanconf::Config::new().expect("Failed to get pacman config")
        });

        &PACMAN_CONFIG
    }

    //---------------------------------------
    // Log functions
    //---------------------------------------
    pub fn log() -> &'static RwLock<Option<String>> {
        static PACMAN_LOG: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| {
            RwLock::new(None)
        });

        &PACMAN_LOG
    }

    pub fn set_log(new_log: Option<String>) {
        let mut pacman_log = Self::log().write().unwrap();

        *pacman_log = new_log;
    }

    //---------------------------------------
    // Cache functions
    //---------------------------------------
    pub fn cache() -> &'static RwLock<Vec<PathBuf>> {
        static PACMAN_CACHE: LazyLock<RwLock<Vec<PathBuf>>> = LazyLock::new(|| {
            RwLock::new(vec![])
        });

        &PACMAN_CACHE
    }

    pub fn set_cache(new_cache: Vec<PathBuf>) {
        let mut pacman_cache = Self::cache().write().unwrap();

        *pacman_cache = new_cache;
    }
}
