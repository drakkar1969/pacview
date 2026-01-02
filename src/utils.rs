use std::sync::{LazyLock, OnceLock, RwLock};
use std::path::PathBuf;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::time::Duration;
use std::fmt::Write as _;

use gtk::{gio, glib, pango};
use gio::{AppInfo, AppLaunchContext};
use gtk::prelude::{AppInfoExtManual, ToValue, WidgetExt};
use pango::{FontDescription, FontMask, Weight};
use sourceview5::{StyleScheme, StyleSchemeManager};

use which::which_global;
use tokio::runtime::Runtime;
use tokio::fs::File;
use tokio_util::io::StreamReader;
use futures_util::TryStreamExt;
use async_compression::tokio::bufread::GzipDecoder;

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
// STRUCT: TokioRuntime
//------------------------------------------------------------------------------
pub struct TokioRuntime;

impl TokioRuntime {
    //---------------------------------------
    // Runtime function
    //---------------------------------------
    pub fn runtime() -> &'static Runtime {
        static RUNTIME: OnceLock<Runtime> = OnceLock::new();

        RUNTIME.get_or_init(|| {
            Runtime::new().expect("Failed to set up tokio runtime")
        })
    }
}

//------------------------------------------------------------------------------
// STRUCT: AsyncCommand
//------------------------------------------------------------------------------
pub struct AsyncCommand;

impl AsyncCommand {
    //---------------------------------------
    // Run function
    //---------------------------------------
    pub async fn run<I, S1, S2>(cmd: S1, args: I) -> io::Result<(Option<i32>, String)>
    where S1: AsRef<OsStr>, I: IntoIterator<Item = S2>, S2: AsRef<OsStr> {
        let output = async_process::Command::new(cmd)
            .args(args)
            .output()
            .await?;

        let stdout = String::from_utf8(output.stdout)
            .map_err(io::Error::other)?;

        Ok((output.status.code(), stdout))
    }

    //---------------------------------------
    // Spawn function
    //---------------------------------------
    pub fn spawn<I, S1, S2>(cmd: S1, args: I) -> io::Result<()>
    where S1: AsRef<OsStr>, I: IntoIterator<Item = S2>, S2: AsRef<OsStr> {
        async_process::Command::new(cmd)
            .args(args)
            .spawn()?;

        Ok(())
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
    pub async fn open_containing_folder(path: &str) {
        let uri = format!("file://{path}");

        if let Some(desktop) = AppInfo::default_for_type("inode/directory", true) {
            let _ = desktop.launch_uris_future(&[&uri], None::<&AppLaunchContext>).await;
        }
    }

    //---------------------------------------
    // Open with default app function
    //---------------------------------------
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

//------------------------------------------------------------------------------
// STRUCT: PangoUtils
//------------------------------------------------------------------------------
pub struct PangoUtils;

impl PangoUtils {
    //---------------------------------------
    // Pango color from style
    //---------------------------------------
    pub fn color_from_style(style: &str) -> (u16, u16, u16, u16) {
        let fc = |color: f32| -> u16 {
            (color * f32::from(u16::MAX)) as u16
        };

        let label = gtk::Label::builder()
            .css_name("texttag")
            .css_classes([style])
            .build();

        let color = label.color();

        (fc(color.red()), fc(color.green()), fc(color.blue()), fc(color.alpha()))
    }

    //-----------------------------------
    // Pango font str to CSS function
    //-----------------------------------
    pub fn font_str_to_css(font_str: &str) -> String {
        let mut css = String::new();

        let font_desc = FontDescription::from_string(font_str);

        let mask = font_desc.set_fields();

        if mask.contains(FontMask::FAMILY)
            && let Some(family) = font_desc.family() {
                write!(css, "font-family: {family}; ").unwrap();
            }

        if mask.contains(FontMask::SIZE) {
            let font_size = font_desc.size()/pango::SCALE;

            write!(css, "font-size: {}pt; ", font_size.max(0)).unwrap();
        }

        if mask.contains(FontMask::WEIGHT) {
            let weight = match font_desc.weight() {
                Weight::Normal => "normal",
                Weight::Bold => "bold",
                Weight::Thin => "100",
                Weight::Ultralight => "200",
                Weight::Light | Weight::Semilight => "300",
                Weight::Book => "400",
                Weight::Medium => "500",
                Weight::Semibold => "600",
                Weight::Ultrabold => "800",
                Weight::Heavy | Weight::Ultraheavy => "900",
                _ => unreachable!()
            };

            write!(css, "font-weight: {weight}; ").unwrap();
        }

        if mask.contains(FontMask::STYLE)
            && let Some((_, value)) = glib::EnumValue::from_value(&font_desc.style()
                .to_value()) {
                write!(css, "font-style: {}; ", value.nick()).unwrap();
            }

        css
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
            .collect::<Vec<StyleScheme>>()
    }
}
