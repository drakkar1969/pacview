use std::sync::{LazyLock, RwLock};
use std::path::{PathBuf, Path};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::process::Stdio;
use std::time::Duration;
use std::env;
use std::collections::HashMap;

use gtk::{gio, glib};
use gio::{AppInfo, AppLaunchContext};
use gtk::prelude::{AppInfoExtManual, WidgetExt};
use sourceview5::{StyleScheme, StyleSchemeManager};

use which::which_global;
use tokio::runtime::Runtime;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio_util::io::StreamReader;
use tokio_util::sync::CancellationToken;
use futures_util::TryStreamExt;
use async_compression::tokio::bufread::GzipDecoder;
use configparser::ini::Ini;
use regex::Regex;

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
// STRUCT: ParuConf
//------------------------------------------------------------------------------
pub struct ParuConf;

impl ParuConf {
    //---------------------------------------
    // Config function
    //---------------------------------------
    fn config() -> &'static Result<Ini, String> {
        static INI: LazyLock<Result<Ini, String>> = LazyLock::new(|| {
            let mut ini = Ini::new();

            env::var("PARU_CONF")
                .map_err(|e| e.to_string())
                .and_then(|var| ini.load(var))
                .or_else(|_| ini.load(glib::user_config_dir().join("paru/paru.conf")))
                .or_else(|_| ini.load(Path::new("/etc/paru.conf")))
                .map(|_| ini)
        });

        &INI
    }

    //---------------------------------------
    // Repo names functions
    //---------------------------------------
    pub fn repo_names() -> Vec<String> {
        Self::config().as_ref()
            .map(|ini| {
                ini.sections()
                    .into_iter()
                    .filter(|section| !["options", "bin", "env"].contains(&section.as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }

    //---------------------------------------
    // Local pkg map functions
    //---------------------------------------
    pub fn local_pkg_map() -> HashMap<String, String> {
        Self::repo_names().into_iter()
            .flat_map(|name| {
                let path = glib::user_cache_dir().join(format!("paru/clone/repo/{name}"));

                fs::read_dir(path)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(move |entry| {
                        (entry.file_name().to_string_lossy().into_owned(), name.clone())
                    })

            })
            .collect()
    }
}

//------------------------------------------------------------------------------
// STRUCT: TokioUtils
//------------------------------------------------------------------------------
pub struct TokioUtils;

impl TokioUtils {
    //---------------------------------------
    // Runtime function
    //---------------------------------------
    pub fn runtime() -> &'static Runtime {
        static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
            Runtime::new().expect("Failed to set up tokio runtime")
        });

        &RUNTIME
    }

    //---------------------------------------
    // Run function
    //---------------------------------------
    pub async fn run<I, S1, S2>(cmd: S1, args: I, token: Option<CancellationToken>) -> io::Result<(Option<i32>, String)>
    where S1: AsRef<OsStr>, I: IntoIterator<Item = S2>, S2: AsRef<OsStr> {
        let cmd_owned = cmd.as_ref().to_os_string();

        let args_owned: Vec<OsString> = args.into_iter()
            .map(|s| s.as_ref().to_os_string())
            .collect();

        Self::runtime().spawn(
            async move {
                // Spawn process
                let mut child = tokio::process::Command::new(cmd_owned)
                    .args(args_owned)
                    .stdout(Stdio::piped())
                    .spawn()?;

                // Get stdout pipe
                let mut stdout_pipe = child.stdout.take().unwrap();

                // Resolve cancellation token
                let cancellation_future = async {
                    if let Some(token) = token {
                        token.cancelled().await;
                    } else {
                        std::future::pending::<()>().await; 
                    }
                };

                tokio::pin!(cancellation_future);

                // Loop: read stdout or wait for process or check for cancellation
                let mut exit_status = None;
                let mut buffer = vec![];

                while exit_status.is_none() {
                    tokio::select! {
                        read = stdout_pipe.read_buf(&mut buffer) => {
                            // EOF
                            if read? == 0 {
                                break;
                            }
                        }
                        status = child.wait() => {
                            exit_status = Some(status?);
                        }
                        () = &mut cancellation_future => {
                            // Kill the process immediately
                            child.kill().await?;

                            // Re-reap the process handle to prevent zombie processes
                            let _ = child.wait().await; 

                            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Process cancelled"));
                        }
                    }
                }

                // Get status code
                let code = match exit_status {
                    Some(status) => status,
                    None => child.wait().await?
                }
                .code();

                // Finish reading stdout
                stdout_pipe.read_to_end(&mut buffer).await?;

                let stdout = String::from_utf8(buffer)
                    .map(|stdout| {
                        static EXPR: LazyLock<Regex> = LazyLock::new(|| {
                            Regex::new(r"\x1b(?:\[[0-9;]*m|\(B)")
                                .expect("Failed to compile Regex")
                        });

                        EXPR.replace_all(&stdout, "").into_owned()
                    })
                    .map_err(io::Error::other)?;

                Ok((code, stdout))
            }
        )
        .await
        .expect("Failed to complete tokio task")
    }

    //---------------------------------------
    // Spawn pipe stdin function
    //---------------------------------------
    pub async fn spawn_pipe_stdin<I, S1, S2>(cmd: S1, args: I, input: &str) -> io::Result<()>
    where S1: AsRef<OsStr>, I: IntoIterator<Item = S2>, S2: AsRef<OsStr> {
        let cmd_owned = cmd.as_ref().to_os_string();

        let args_owned: Vec<OsString> = args.into_iter()
            .map(|s| s.as_ref().to_os_string())
            .collect();

        let input_owned = input.to_owned();

        Self::runtime().spawn(
            async move {
                let mut child = tokio::process::Command::new(cmd_owned)
                    .args(args_owned)
                    .stdin(Stdio::piped())
                    .spawn()?;

                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(input_owned.as_bytes()).await?;
                }

                Ok(())
            }
        )
        .await
        .expect("Failed to complete tokio task")
    }
}

//------------------------------------------------------------------------------
// STRUCT: AppInfoExt
//------------------------------------------------------------------------------
pub struct AppInfoExt;

impl AppInfoExt {
    //---------------------------------------
    // Open containing folder function
    //---------------------------------------
    #[allow(clippy::future_not_send)]
    pub async fn open_containing_folder(path: &str) {
        let uri = format!("file://{path}");

        if let Some(desktop) = AppInfo::default_for_type("inode/directory", true) {
            let _ = desktop.launch_uris_future(&[&uri], None::<&AppLaunchContext>).await;
        }
    }

    //---------------------------------------
    // Open with default app function
    //---------------------------------------
    #[allow(clippy::future_not_send)]
    pub async fn open_with_default_app(path: &str) {
        let uri = format!("file://{path}");
        let path = path.to_owned();

        if AppInfo::launch_default_for_uri_future(&uri, None::<&AppLaunchContext>)
            .await
            .is_err() {
                Self::open_containing_folder(&path).await;
            }
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
    pub fn path() -> Option<&'static PathBuf> {
        static AUR_FILE: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
            let cache_dir = glib::user_cache_dir().join("pacview");

            fs::create_dir_all(&cache_dir)
                .map(|()| cache_dir.join("aur_packages"))
                .ok()
        });

        AUR_FILE.as_ref()
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
        let aur_file = Self::path()
            .ok_or_else(|| io::Error::other("Failed to retrieve AUR database path"))?;

        // Spawn tokio task to download AUR file
        TokioUtils::runtime().spawn(
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

                Ok(())
            }
        )
        .await
        .expect("Failed to complete tokio task")
    }
}

//------------------------------------------------------------------------------
// STRUCT: Color
//------------------------------------------------------------------------------
pub struct Color;

impl Color {
    //---------------------------------------
    // Pango color from style
    //---------------------------------------
    pub fn pango_color_from_style(style: &str) -> (u16, u16, u16, u16) {
        let fc = |color: f32| -> u16 {
            (color * f32::from(u16::MAX)) as u16
        };

        let color = gtk::Label::builder()
            .css_name("texttag")
            .css_classes([style])
            .build()
            .color();

        (fc(color.red()), fc(color.green()), fc(color.blue()), fc(color.alpha()))
    }

    //---------------------------------------
    // Cairo color from style
    //---------------------------------------
    pub fn cairo_color_from_style(style: &str) -> (f64, f64, f64, f64) {
        let color = gtk::Label::builder()
            .css_name("texttag")
            .css_classes([style])
            .build()
            .color();

        (
            f64::from(color.red()),
            f64::from(color.green()),
            f64::from(color.blue()),
            f64::from(color.alpha())
        )
    }
}

//------------------------------------------------------------------------------
// STRUCT: StyleSchemes
//------------------------------------------------------------------------------
pub struct StyleSchemes;

impl StyleSchemes {
    //-----------------------------------
    // Is variant dark functions
    //-----------------------------------
    pub fn is_variant_dark(scheme: &StyleScheme) -> bool {
        scheme.metadata("variant").is_some_and(|variant| variant == "dark")
    }

    pub fn is_variant_dark_by_id(id: &str) -> bool {
        StyleSchemeManager::default()
            .scheme(id)
            .is_some_and(|scheme| Self::is_variant_dark(&scheme))
    }

    //-----------------------------------
    // Variant function
    //-----------------------------------
    pub fn variant_id(id: &str) -> Option<glib::GString> {
        let scheme_manager = StyleSchemeManager::default();

        let scheme = scheme_manager.scheme(id)?;

        let variant = scheme.metadata("variant")?;

        if variant == "dark" {
            scheme.metadata("light-variant")
        } else {
            scheme.metadata("dark-variant")
        }
    }

    //-----------------------------------
    // Schemes function
    //-----------------------------------
    pub fn schemes(dark: bool) -> Vec<StyleScheme> {
        let scheme_manager = StyleSchemeManager::default();

        scheme_manager
            .scheme_ids()
            .iter()
            .filter_map(|id| {
                scheme_manager.scheme(id)
                    .filter(|scheme| Self::is_variant_dark(scheme) == dark)
            })
            .collect()
    }
}
