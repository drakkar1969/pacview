use std::sync::{LazyLock, RwLock};
use std::path::PathBuf;
use std::fs;
use std::io;
use std::time::Duration;

use gtk::glib;

use which::which_global;
use tokio::fs::File;
use tokio_util::io::StreamReader;
use futures_util::TryStreamExt;
use async_compression::tokio::bufread::GzipDecoder;

use crate::utils::TokioRuntime;

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

//------------------------------------------------------------------------------
// STRUCT: AurDBFile
//------------------------------------------------------------------------------
pub struct AurDBFile;

impl AurDBFile {
    //---------------------------------------
    // Path function
    //---------------------------------------
    pub fn path() -> &'static Option<PathBuf> {
        static AUR_FILE: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
            let cache_dir = glib::user_cache_dir().join("pacview");

            fs::create_dir_all(&cache_dir)
                .map(|()| cache_dir.join("aur_packages"))
                .ok()
        });

        &AUR_FILE
    }

    //---------------------------------------
    // Out of date function
    //---------------------------------------
    pub fn out_of_date(max_age: u64) -> bool {
        // Get AUR package names file age
        let file_time = Self::path().as_ref()
            .and_then(|aur_file| fs::metadata(aur_file).ok())
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|file_time| {
                let now = std::time::SystemTime::now();

                now.duration_since(file_time).ok()
            });

        file_time.is_none_or(|time| time >= Duration::from_hours(max_age))
    }

    //---------------------------------------
    // Download async function
    //---------------------------------------
    pub async fn download() -> Result<(), io::Error> {
        let aur_file = Self::path().as_ref()
            .ok_or_else(|| io::Error::other("Failed to retrieve AUR database path"))?;

        // Spawn tokio task to download AUR file
        TokioRuntime::runtime().spawn(
            async move {
                let response = reqwest::Client::new()
                    .get("https://aur.archlinux.org/packages.gz")
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
                    .map_err(io::Error::other)?;

                let stream = response
                    .bytes_stream()
                    .map_err(io::Error::other);

                let stream_reader = StreamReader::new(stream);
                let mut decoder = GzipDecoder::new(stream_reader);

                let mut out_file = File::create(aur_file).await?;

                tokio::io::copy(&mut decoder, &mut out_file).await?;

                Ok::<(), io::Error>(())
            }
        )
        .await
        .expect("Failed to complete tokio task")
    }
}
