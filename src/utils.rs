use std::sync::{LazyLock, RwLock};
use std::path::{PathBuf, Path};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::process::Stdio;
use std::time::Duration;
use std::env;
use std::collections::HashMap;

use gtk::{gio, glib};
use gio::{AppInfo, AppLaunchContext};
use gtk::prelude::{AppInfoExtManual, ListModelExt, Cast, CastNone, IsA, ObjectExt};
use sourceview5::{StyleScheme, StyleSchemeManager};

use walkdir::WalkDir;
use which::which_global;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio_util::io::StreamReader;
use tokio_util::sync::CancellationToken;
use futures_util::TryStreamExt;
use async_compression::tokio::bufread::GzipDecoder;
use configparser::ini::Ini;
use url::Url;

//------------------------------------------------------------------------------
// STRUCT: Paths
//------------------------------------------------------------------------------
pub struct Paths;

impl Paths {
    //---------------------------------------
    // Cache dir function
    //---------------------------------------
    pub fn cache_dir() -> PathBuf {
        glib::user_cache_dir().join(env!("CARGO_PKG_NAME"))
    }

    //---------------------------------------
    // Paru bin path function
    //---------------------------------------
    pub fn paru_bin() -> which::Result<PathBuf> {
        which_global("paru")
    }

    //---------------------------------------
    // Paccat bin path function
    //---------------------------------------
    pub fn paccat_bin() -> which::Result<PathBuf> {
        which_global("paccat")
    }

    //---------------------------------------
    // Meld bin path function
    //---------------------------------------
    pub fn meld_bin() -> which::Result<PathBuf> {
        which_global("meld")
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
    pub fn config() -> &'static RwLock<pacmanconf::Config> {
        static PACMAN_CONFIG: LazyLock<RwLock<pacmanconf::Config>> = LazyLock::new(|| {
            RwLock::new(pacmanconf::Config::new().expect("Failed to get pacman config"))
        });

        &PACMAN_CONFIG
    }

    //---------------------------------------
    // Config path function
    //---------------------------------------
    pub fn config_path() -> &'static RwLock<String> {
        static CONFIG_PATH: LazyLock<RwLock<String>> = LazyLock::new(|| {
            RwLock::new("/etc/pacman.conf".into())
        });

        &CONFIG_PATH
    }

    //---------------------------------------
    // Default config path for root function
    //---------------------------------------
    pub fn default_config_path_for_root(root_dir: &str) -> String {
        Path::new(root_dir)
            .join("etc/pacman.conf")
            .display()
            .to_string()
    }

    //---------------------------------------
    // Set root dir function
    //---------------------------------------
    pub fn set_root_dir(root_dir: &str, config_path: Option<&str>) -> Result<(), pacmanconf::Error> {
        let config_path = config_path
            .map_or_else(|| Self::default_config_path_for_root(root_dir), ToOwned::to_owned);

        *Self::config().write().unwrap() = pacmanconf::Config::options()
            .root_dir(root_dir)
            .pacman_conf(&config_path)
            .read()?;

        let mut path = Self::config_path().write().unwrap();

        config_path.clone_into(&mut path);

        Ok(())
    }

    //---------------------------------------
    // Is default root dir function
    //---------------------------------------
    pub fn is_default_root_dir() -> bool {
        Self::config().read().unwrap().root_dir == "/"
    }
}

//------------------------------------------------------------------------------
// STRUCT: PkgbuildRepos
//------------------------------------------------------------------------------
#[derive(Debug)]
pub struct PkgbuildPkgInfo {
    pub repo: String,
    pub path: PathBuf
}

pub struct PkgbuildRepos;

impl PkgbuildRepos {
    //---------------------------------------
    // Paru config helper function
    //---------------------------------------
    #[allow(clippy::ref_option)]
    fn paru_config() -> &'static Option<Ini> {
        static INI: LazyLock<Option<Ini>> = LazyLock::new(|| {
            let paths = [
                env::var_os("PARU_CONF").map(Into::into),
                Some(glib::user_config_dir().join("paru/paru.conf")),
                Some(Path::new("/etc/paru.conf").to_path_buf())
            ];

            for path in paths.into_iter().flatten() {
                let mut ini = Ini::new();

                if ini.load(path).is_ok() {
                    return Some(ini);
                }
            }

            None
        });

        &INI
    }

    //---------------------------------------
    // Fetcher helper function
    //---------------------------------------
    fn fetcher() -> &'static aur_fetch::Fetch {
        static FETCHER: LazyLock<aur_fetch::Fetch> = LazyLock::new(|| {
            aur_fetch::Fetch::with_cache_dir(Paths::cache_dir())
        });

        &FETCHER
    }

    //---------------------------------------
    // Clone dir exists function
    //---------------------------------------
    pub fn clone_dir_exists() -> bool {
        Self::fetcher().clone_dir.try_exists().is_ok_and(|res| res)
    }

    //---------------------------------------
    // Repos function
    //---------------------------------------
    pub fn repos() -> &'static Vec<aur_fetch::Repo> {
        static REPOS: LazyLock<Vec<aur_fetch::Repo>> = LazyLock::new(|| {
            let Some(paru_config) = PkgbuildRepos::paru_config().as_ref() else {
                return vec![];
            };

            paru_config.sections()
                .into_iter()
                .filter(|section| !["options", "bin", "env"].contains(&section.as_str()))
                .filter_map(|section| {
                    paru_config.get(&section, "url")
                        .map(|mut url| {
                            if let Some(path) = paru_config.get(&section, "path") {
                                if !url.ends_with('/') && !path.starts_with('/') {
                                    url.push('/');
                                }

                                url.push_str(&path);
                            }

                            url
                        })
                        .or_else(|| paru_config.get(&section, "path"))
                        .and_then(|url| Url::parse(&url).ok())
                        .map(|url| aur_fetch::Repo { url, name: section })
                })
                .collect()
        });

        &REPOS
    }

    //---------------------------------------
    // Init pkg map helper function
    //---------------------------------------
    fn init_pkg_map() -> HashMap<String, PkgbuildPkgInfo> {
        let clone_dir = Self::fetcher().clone_dir.as_path();

        Self::repos().iter()
            .map(|repo| repo.name.as_str())
            .flat_map(|repo_name| {
                let path = clone_dir.join(repo_name);

                WalkDir::new(path)
                    .min_depth(1)
                    .into_iter()
                    .filter_entry(|entry| {
                        entry.file_type().is_dir() && entry.file_name() != ".git"
                    })
                    .flatten()
                    .filter(|entry| {
                        entry.path().join(".SRCINFO").try_exists().is_ok_and(|res| res)
                    })
                    .map(|entry| {
                        (
                            entry.file_name().to_string_lossy().into_owned(),
                            PkgbuildPkgInfo {
                                repo: repo_name.to_owned(),
                                path: entry.into_path()
                            }
                        )
                    })
            })
            .collect()
    }

    //---------------------------------------
    // Fetched pkg map function
    //---------------------------------------
    pub fn fetched_pkg_map() -> &'static RwLock<HashMap<String, PkgbuildPkgInfo>> {
        static MAP: LazyLock<RwLock<HashMap<String, PkgbuildPkgInfo>>> = LazyLock::new(|| {
            RwLock::new(PkgbuildRepos::init_pkg_map())
        });

        &MAP
    }

    //---------------------------------------
    // Fetch remote function
    //---------------------------------------
    #[allow(clippy::needless_pass_by_value)]
    pub fn fetch_remote(token: CancellationToken) -> aur_fetch::Result<Vec<String>> {
        // Partition repos into local and remote
        let mut remote_repos: Vec<aur_fetch::Repo> = Self::repos().iter()
            .map(|repo| aur_fetch::Repo { url: repo.url.clone(), name: repo.name.clone() })
            .collect();

        let local_repos: Vec<aur_fetch::Repo> = remote_repos
            .extract_if(.., |repo| repo.url.scheme() == "file")
            .collect();

        // Fetch remote repos
        let fetch = Self::fetcher();

        if token.is_cancelled() {
            return Err(aur_fetch::Error::Io(io::Error::other("Cancelled by user")));
        }

        let mut repo_names = fetch.download_repos_cb(&remote_repos, |_| {})?;

        // Merge remote repos
        if token.is_cancelled() {
            return Err(aur_fetch::Error::Io(io::Error::other("Cancelled by user")));
        }

        fetch.merge(&repo_names)?;

        // Copy local repos to clone dir
        let clone_dir = fetch.clone_dir.as_path();

        let copy_options = fs_extra::dir::CopyOptions {
            copy_inside: true,
            .. fs_extra::dir::CopyOptions::default()
        };

        if fs::create_dir_all(clone_dir).is_ok() {
            for repo in local_repos {
                let dest_path = clone_dir.join(&repo.name);

                // Copy local repo
                if token.is_cancelled() {
                    return Err(aur_fetch::Error::Io(io::Error::other("Cancelled by user")));
                }

                if dest_path.try_exists().is_ok_and(|res| res) {
                    let _ = fs::remove_dir_all(&dest_path);
                }

                if fs_extra::copy_items(&[repo.url.path()], &dest_path, &copy_options).is_ok() {
                    repo_names.push(repo.name);
                }
            }
        }

        // Re-init fetched pkg map
        *Self::fetched_pkg_map().write().unwrap() = Self::init_pkg_map();

        Ok(repo_names)
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
    fn path() -> PathBuf {
        Paths::cache_dir().join("aur_packages")
    }

    //---------------------------------------
    // Exists function
    //---------------------------------------
    pub fn exists() -> bool {
        Self::path().try_exists().is_ok_and(|result| result)
    }

    //---------------------------------------
    // Load function
    //--------------------------------------
    pub fn load() -> String {
        fs::read_to_string(Self::path()).unwrap_or_default()
    }

    //---------------------------------------
    // Out of date function
    //---------------------------------------
    pub fn out_of_date(max_age: u64) -> bool {
        // Get AUR package names file age
        let file_age = fs::metadata(Self::path()).ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|file_time| {
                let now = std::time::SystemTime::now();

                now.duration_since(file_time).ok()
            });

        file_age.is_none_or(|age| age >= Duration::from_hours(max_age))
    }

    //---------------------------------------
    // Download async function
    //---------------------------------------
    pub async fn download(token: CancellationToken) -> Result<(), io::Error> {
        // Spawn tokio task to download AUR file
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(io::Error::other)?;

        // Get response
        let url = "https://aur.archlinux.org/packages.gz";

        let response = tokio::select! {
            () = token.cancelled() => Err(io::Error::other("Cancelled by user")),
            result = client.get(url).send() => result.map_err(io::Error::other)
        }?;

        // Write response to file
        let stream = response.bytes_stream().map_err(io::Error::other);
        let stream_reader = StreamReader::new(stream);
        let mut decoder = GzipDecoder::new(stream_reader);
        let mut out_file = File::create(Self::path()).await?;

        tokio::select! {
            () = token.cancelled() => {
                let _ = tokio::fs::remove_file(Self::path()).await;

                Err(io::Error::other("Cancelled by user"))
            }
            result = tokio::io::copy(&mut decoder, &mut out_file) => result
        }?;

        Ok(())
    }
}

//------------------------------------------------------------------------------
// STRUCT: TokioCommand
//------------------------------------------------------------------------------
pub struct TokioCommand;

impl TokioCommand {
    //---------------------------------------
    // Output function
    //---------------------------------------
    pub async fn output<I, S1, S2>(cmd: S1, args: I, token: CancellationToken, strip_ansi: bool)
    -> io::Result<(Option<i32>, String, Option<String>)>
    where
        S1: AsRef<OsStr>,
        S2: AsRef<OsStr>,
        I: IntoIterator<Item = S2>
    {
        // Spawn process
        let mut child = tokio::process::Command::new(cmd)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Get stdout/stderr pipes
        let mut stdout_pipe = child.stdout.take().unwrap();
        let mut stderr_pipe = child.stderr.take().unwrap();

        // Loop: read stdout or wait for process or check for cancellation
        let mut exit_status = None;
        let mut stdout_buffer = vec![];

        while exit_status.is_none() {
            tokio::select! {
                read = stdout_pipe.read_buf(&mut stdout_buffer) => {
                    // EOF
                    if read? == 0 {
                        break;
                    }
                }
                status = child.wait() => {
                    exit_status = Some(status?);
                }
                () = token.cancelled() => {
                    // Kill the process immediately
                    child.kill().await?;

                    // Re-reap the process handle to prevent zombie processes
                    let _ = child.wait().await;

                    return Err(io::Error::other("Cancelled by user"));
                }
            }
        }

        // Get exit code
        let code = match exit_status {
            Some(status) => status,
            None => child.wait().await?
        }
        .code();

        // Read stderr if non-zero exit code
        let stderr = if code == Some(0) {
            None
        } else {
            let mut stderr = String::new();

            stderr_pipe.read_to_string(&mut stderr).await?;

            if stderr.is_empty() {
                stderr = "unknown error".into();
            }

            Some(stderr)
        };

        // Finish reading stdout
        stdout_pipe.read_to_end(&mut stdout_buffer).await?;

        // Strip ANSI codes from stdout
        let stdout = if strip_ansi {
            String::from_utf8(strip_ansi_escapes::strip(stdout_buffer))
        } else {
            String::from_utf8(stdout_buffer)
        }
        .map_err(io::Error::other)?;

        Ok((code, stdout, stderr))
    }

    //---------------------------------------
    // Spawn pipe stdin function
    //---------------------------------------
    pub async fn spawn_pipe_stdin<I, S1, S2>(cmd: S1, args: I, input: &str) -> io::Result<()>
    where
        S1: AsRef<OsStr>,
        S2: AsRef<OsStr>,
        I: IntoIterator<Item = S2>
    {
        let mut child = tokio::process::Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes()).await?;
        }

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
// STRUCT: StyleSchemes
//------------------------------------------------------------------------------
pub struct StyleSchemes;

impl StyleSchemes {
    //-----------------------------------
    // Is variant dark helper function
    //-----------------------------------
    fn is_variant_dark(scheme: &StyleScheme) -> bool {
        scheme.metadata("variant").is_some_and(|variant| variant == "dark")
    }

    //-----------------------------------
    // Variant id helper function
    //-----------------------------------
    fn variant_id(id: &str) -> Option<glib::GString> {
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

    //-----------------------------------
    // Scheme function
    //-----------------------------------
    pub fn scheme(id: &str, dark: bool) -> Option<StyleScheme> {
        let scheme_manager = StyleSchemeManager::default();

        scheme_manager.scheme(id)
            .filter(|scheme| Self::is_variant_dark(scheme) == dark)
            .or_else(|| {
                let variant_id = Self::variant_id(id)?;

                scheme_manager.scheme(&variant_id)
            })
    }

    //-----------------------------------
    // Scheme matches id function
    //-----------------------------------
    pub fn scheme_matches_id(scheme: &StyleScheme, id: &str) -> bool {
        scheme.id() == id
            || Self::variant_id(&scheme.id())
                .is_some_and(|variant_id| variant_id == id)
    }
}

//------------------------------------------------------------------------------
// TRAIT: ListStoreFind
//------------------------------------------------------------------------------
pub trait ListStoreFind {
    fn find_with<F, T>(&self, func: F) -> Option<T>
    where
        F: FnMut(&T) -> bool + Clone,
        T: IsA<glib::Object>;
}

impl ListStoreFind for gio::ListStore {
    //-----------------------------------
    // Find with function
    //-----------------------------------
    fn find_with<F, T>(&self, mut func: F) -> Option<T>
    where
        F: FnMut(&T) -> bool + Clone,
        T: IsA<glib::Object>
    {
        let index = self.find_with_equal_func(|obj| {
            let type_obj = obj.downcast_ref::<T>()
                .expect(&format!("Failed to downcast to '{}'", obj.type_()));

            func(type_obj)
        });

        index.and_then(|index| self.item(index).and_downcast::<T>())
    }
}
