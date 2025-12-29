use std::borrow::Cow;
use std::cell::{RefCell, OnceCell};
use std::sync::LazyLock;
use std::cmp::Ordering;

use gtk::{glib, gio};
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use alpm_utils::DbListExt;
use regex::Regex;
use size::Size;
use rayon::prelude::*;
use sourceview5::prelude::ListModelExtManual;
use tokio::sync::OnceCell as TokioOnceCell;

use crate::vars::{paths, pacman};
use crate::pkg_data::{PkgData, PkgFlags, PkgValidation};

//------------------------------------------------------------------------------
// STRUCT: PkgBackup
//------------------------------------------------------------------------------
#[derive(Debug)]
pub struct PkgBackup {
    filename: String,
    hash: String,
    package: String
}

impl PkgBackup {
    fn new(filename: &str, hash: &str, package: &str) -> Self {
        Self {
            filename: filename.to_owned(),
            hash: hash.to_owned(),
            package: package.to_owned()
        }
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn package(&self) -> &str {
        &self.package
    }
}

//------------------------------------------------------------------------------
// STRUCT: PkgHashes
//------------------------------------------------------------------------------
#[derive(Default, Debug)]
pub struct PkgHashes {
    base64_sig: Option<String>,
    sha256sum: Option<String>,
    md5sum: Option<String>
}

impl PkgHashes {
    pub fn new(base64_sig: Option<&str>, sha256sum: Option<&str>, md5sum: Option<&str>) -> Self {
        Self {
            base64_sig: base64_sig.map(ToOwned::to_owned),
            sha256sum: sha256sum.map(ToOwned::to_owned),
            md5sum: md5sum.map(ToOwned::to_owned)
        }
    }

    pub fn base64_sig(&self) -> Option<&str> {
        self.base64_sig.as_deref()
    }

    pub fn sha256sum(&self) -> Option<&str> {
        self.sha256sum.as_deref()
    }

    pub fn md5sum(&self) -> Option<&str> {
        self.md5sum.as_deref()
    }
}

//------------------------------------------------------------------------------
// MODULE: PkgObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::PkgObject)]
    pub struct PkgObject {
        // Read-write properties
        #[property(get, set, nullable)]
        update_version: RefCell<Option<String>>,

        // Read-only properties with getter
        #[property(name = "flags", get = Self::flags, type = PkgFlags)]
        #[property(name = "version", get = Self::version, type = String)]

        // Read-only properties from data fields
        #[property(name = "name", get, type = String, member = name)]
        #[property(name = "repository", get, type = String, member = repository)]
        pub(super) data: OnceCell<PkgData>,

        // Read only fields
        pub(super) required_by: OnceCell<Vec<String>>,
        pub(super) optional_for: OnceCell<Vec<String>>,

        pub(super) files: OnceCell<Vec<String>>,
        pub(super) backup: OnceCell<Vec<PkgBackup>>,

        pub(super) log: TokioOnceCell<Vec<String>>,
        pub(super) cache: TokioOnceCell<Vec<String>>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PkgObject {
        const NAME: &'static str = "PkgObject";
        type Type = super::PkgObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for PkgObject {}

    impl PkgObject {
        //---------------------------------------
        // Read-only property getters
        //---------------------------------------
        fn flags(&self) -> PkgFlags {
            let flags = self.data.get().unwrap().flags;

            self.update_version.borrow().as_ref()
                .map_or(flags, |_| flags | PkgFlags::UPDATES)
        }

        fn version(&self) -> String {
            let version = &self.data.get().unwrap().version;

            self.update_version.borrow().as_ref()
                .map_or_else(|| version.to_owned(), |update_version| {
                    version.to_owned() + " \u{2192} " + update_version
                })
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PkgObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PkgObject(ObjectSubclass<imp::PkgObject>);
}

impl PkgObject {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(data: PkgData) -> Self {
        let pkg: Self = glib::Object::builder().build();

        pkg.imp().data.set(data).unwrap();

        pkg.connect_update_version_notify(|pkg| {
            pkg.notify_flags();
            pkg.notify_version();
        });

        pkg
    }

    //---------------------------------------
    // Public data field getters
    //---------------------------------------
    #[inline]
    fn data(&self) -> &PkgData {
        self.imp().data.get().unwrap()
    }
    
    pub fn base(&self) -> &str {
        &self.data().base
    }

    pub fn description(&self) -> &str {
        &self.data().description
    }

    pub fn popularity(&self) -> &str {
        &self.data().popularity
    }

    pub fn out_of_date(&self) -> i64 {
        self.data().out_of_date
    }

    pub fn out_of_date_string(&self) -> Cow<'_, str> {
        Self::date_to_string(self.data().out_of_date, "%d %B %Y %H:%M")
    }

    pub fn url(&self) -> &str {
        &self.data().url
    }

    pub fn package_url(&self) -> Cow<'_, str> {
        let data = self.data();

        let repo = &data.repository;

        if repo == "aur" {
            return Cow::Owned(format!("https://aur.archlinux.org/packages/{}", data.name));
        }

        if pacman::config().repos.iter().any(|r| &r.name == repo) {
            return Cow::Owned(format!(
                "https://www.archlinux.org/packages/{}/{}/{}/",
                repo, data.architecture, data.name
            ));
        }

        Cow::Borrowed("")
    }

    pub fn pkgbuild_urls(&self) -> (String, String) {
        let data = self.data();
        let name = if data.base.is_empty() { &data.name } else { &data.base };
        let repo = &data.repository;

        match repo.as_str() {
            "aur" => {
                let domain = "https://aur.archlinux.org/cgit/aur.git";

                let url = format!("{domain}/tree/PKGBUILD?h={name}");
                let raw_url = format!("{domain}/plain/PKGBUILD?h={name}");

                (url, raw_url)
            }
            _ if pacman::config().repos.iter().any(|r| &r.name == repo) => {
                let domain = "https://gitlab.archlinux.org/archlinux/packaging/packages";

                let url = format!("{domain}/{name}/-/blob/main/PKGBUILD");
                let raw_url = format!("{domain}/{name}/-/raw/main/PKGBUILD");

                (url, raw_url)
            }
            "local" => {
                (String::new(), String::new())
            }
            _ => {
                let raw_url = paths::paru().as_ref().ok()
                    .map_or_else(String::new, |_| {
                        glib::user_cache_dir()
                            .join(format!("paru/clone/repo/{repo}/{name}/PKGBUILD"))
                            .display()
                            .to_string()
                    });

                let url = format!("file://{raw_url}");

                (url, raw_url)
            }
        }
    }

    pub fn status(&self) -> &str {
        match self.data().flags {
            PkgFlags::EXPLICIT => "explicit",
            PkgFlags::DEPENDENCY => "dependency",
            PkgFlags::OPTIONAL => "optional",
            PkgFlags::ORPHAN => "orphan",
            _ => "not installed"
        }
    }

    pub fn status_icon(&self) -> &str {
        match self.data().flags {
            PkgFlags::EXPLICIT => "pkg-explicit",
            PkgFlags::DEPENDENCY => "pkg-dependency",
            PkgFlags::OPTIONAL => "pkg-optional",
            PkgFlags::ORPHAN => "pkg-orphan",
            _ => ""
        }
    }

    pub fn status_icon_symbolic(&self) -> &str {
        match self.data().flags {
            PkgFlags::EXPLICIT => "status-explicit-symbolic",
            PkgFlags::DEPENDENCY => "status-dependency-symbolic",
            PkgFlags::OPTIONAL => "status-optional-symbolic",
            PkgFlags::ORPHAN => "status-orphan-symbolic",
            _ => ""
        }
    }

    pub fn licenses(&self) -> &[String] {
        &self.data().licenses
    }

    pub fn groups(&self) -> &[String] {
        &self.data().groups
    }

    pub fn depends(&self) -> &[String] {
        &self.data().depends
    }

    pub fn optdepends(&self) -> &[String] {
        &self.data().optdepends
    }

    pub fn makedepends(&self) -> &[String] {
        &self.data().makedepends
    }

    pub fn provides(&self) -> &[String] {
        &self.data().provides
    }

    pub fn conflicts(&self) -> &[String] {
        &self.data().conflicts
    }

    pub fn replaces(&self) -> &[String] {
        &self.data().replaces
    }

    pub fn architecture(&self) -> &str {
        &self.data().architecture
    }

    pub fn packager(&self) -> &str {
        &self.data().packager
    }

    pub fn build_date(&self) -> i64 {
        self.data().build_date
    }

    pub fn build_date_string(&self) -> Cow<'_, str> {
        Self::date_to_string(self.data().build_date, "%d %B %Y %H:%M")
    }

    pub fn install_date(&self) -> i64 {
        self.data().install_date
    }

    pub fn install_date_string(&self) -> Cow<'_, str> {
        Self::date_to_string(self.data().install_date, "%d %B %Y %H:%M")
    }

    pub fn download_size(&self) -> i64 {
        self.data().download_size
    }

    pub fn download_size_string(&self) -> String {
        Size::from_bytes(self.data().download_size).to_string()
    }

    pub fn install_size(&self) -> i64 {
        self.data().install_size
    }

    pub fn install_size_string(&self) -> String {
        Size::from_bytes(self.data().install_size).to_string()
    }

    pub fn has_script(&self) -> &str {
        &self.data().has_script
    }

    pub fn validation(&self) -> PkgValidation {
        self.data().validation
    }

    //---------------------------------------
    // Alpm handle associated function
    //---------------------------------------
    pub fn with_alpm_handle<F, R>(f: F) -> R
    where F: FnOnce(&RefCell<Option<alpm::Alpm>>) -> R {
        thread_local! {
            static ALPM_HANDLE: RefCell<Option<alpm::Alpm>> = const { RefCell::new(None) };
        }
        
        ALPM_HANDLE.with(f)
    }

    //---------------------------------------
    // Get alpm package helper functions
    //---------------------------------------
    fn pkg<'a>(&self, handle: &'a alpm::Alpm) -> Option<&'a alpm::Package> {
        let data = self.data();

        if data.flags.intersects(PkgFlags::INSTALLED) {
            handle.localdb().pkg(data.name.as_str()).ok()
        } else {
            handle.syncdbs().pkg(data.name.as_str()).ok()
        }
    }

    fn sync_pkg<'a>(&self, handle: &'a alpm::Alpm) -> Option<&'a alpm::Package> {
        let data = self.data();

        handle.syncdbs().pkg(data.name.as_str()).ok()
    }

    //---------------------------------------
    // Public getters from alpm package
    //---------------------------------------
    pub fn required_by(&self) -> &[String] {
        self.imp().required_by.get_or_init(|| {
            Self::with_alpm_handle(|handle| {
                handle.borrow().as_ref()
                    .and_then(|handle| self.pkg(handle))
                    .map(|pkg| {
                        let mut required_by: Vec<String> = pkg.required_by()
                            .into_iter()
                            .collect();

                        required_by.sort_unstable();

                        required_by
                    })
                    .unwrap_or_default()
            })
        })
    }

    pub fn optional_for(&self) -> &[String] {
        self.imp().optional_for.get_or_init(|| {
            Self::with_alpm_handle(|handle| {
                handle.borrow().as_ref()
                    .and_then(|handle| self.pkg(handle))
                    .map(|pkg| {
                        let mut optional_for: Vec<String> = pkg.optional_for()
                            .into_iter()
                            .collect();

                        optional_for.sort_unstable();

                        optional_for
                    })
                    .unwrap_or_default()
            })
        })
    }

    pub fn files(&self) -> &[String] {
        self.imp().files.get_or_init(|| {
            Self::with_alpm_handle(|handle| {
                handle.borrow().as_ref()
                    .and_then(|handle| self.pkg(handle))
                    .map(|pkg| {
                        let root_dir = &pacman::config().root_dir;

                        let mut files: Vec<String> = pkg.files().files()
                            .iter()
                            .map(|file| {
                                let mut path = root_dir.to_owned();
                                path.push_str(&String::from_utf8_lossy(file.name()));
                                path
                            })
                            .collect();

                        files.par_sort_unstable();

                        files
                    })
                    .unwrap_or_default()
            })
        })
    }

    pub fn backup(&self) -> &[PkgBackup] {
        self.imp().backup.get_or_init(|| {
            Self::with_alpm_handle(|handle| {
                handle.borrow().as_ref()
                    .and_then(|handle| self.pkg(handle))
                    .map(|pkg| {
                        let root_dir = &pacman::config().root_dir;
                        let pkg_name = self.name();

                        let mut backup: Vec<PkgBackup> = pkg.backup().iter()
                            .map(|backup| {
                                let mut path = root_dir.to_owned();
                                path.push_str(backup.name());

                                PkgBackup::new(&path, backup.hash(), &pkg_name)
                            })
                            .collect();

                        backup.sort_unstable_by(|backup_a, backup_b| {
                            backup_a.filename.partial_cmp(&backup_b.filename).unwrap_or(Ordering::Equal)
                        });

                        backup
                    })
                    .unwrap_or_default()
            })
        })
    }

    pub fn hashes(&self) -> PkgHashes {
        Self::with_alpm_handle(|handle| {
            handle.borrow().as_ref()
                .and_then(|handle| self.sync_pkg(handle))
                .map(|pkg| PkgHashes::new(pkg.base64_sig(), pkg.sha256sum(), pkg.md5sum()))
                .unwrap_or_default()
        })
    }

    //---------------------------------------
    // Public future getters from alpm package
    //---------------------------------------
    pub async fn log_future(&self) -> &Vec<String> {
        self.imp().log.get_or_init(async || {
            static EXPR: LazyLock<Regex> = LazyLock::new(|| {
                Regex::new(r"\[(.+?)T(.+?)\+.+?\] \[ALPM\] (installed|removed|upgraded|downgraded) (.+?) (.+)").expect("Failed to compile Regex")
            });

            let pkg_name = self.name();

            gio::spawn_blocking(move || {
                let pacman_log = pacman::log().read().unwrap();

                pacman_log.as_ref().map_or(vec![], |log| {
                    log.lines().rev()
                        .filter(|&line| line.contains(&pkg_name))
                        .filter_map(|line| {
                            let caps = EXPR.captures(line)?;

                            (caps[4] == pkg_name).then(|| {
                                format!("[{}  {}]  {} {} {}", &caps[1], &caps[2], &caps[3], &caps[4], &caps[5])
                            })
                    })
                    .collect::<Vec<String>>()
                })
            })
            .await
            .expect("Failed to complete task")
        })
        .await
    }

    pub async fn cache_future(&self) -> &Vec<String> {
        self.imp().cache.get_or_init(async || {
            let pkg_name = self.name();

            gio::spawn_blocking(move || {
                let pacman_cache = pacman::cache().read().unwrap();

                pacman_cache.iter()
                    .filter(|&path| {
                        path.file_name()
                            .and_then(|filename| filename.to_str())
                            .filter(|&filename| {
                                filename.rsplitn(4, '-').last()
                                    .is_some_and(|name| name == pkg_name)
                            })
                            .filter(|&filename| filename.ends_with(".pkg.tar.zst"))
                            .is_some()
                    })
                    .map(|path| path.display().to_string())
                    .collect::<Vec<String>>()
            })
            .await
            .expect("Failed to complete task")
        })
        .await
    }

    //---------------------------------------
    // Date to string associated function
    //---------------------------------------
    pub fn date_to_string(date: i64, format: &str) -> Cow<'_, str> {
        if date == 0 {
            Cow::Borrowed("")
        } else {
            Cow::Owned(glib::DateTime::from_unix_local(date)
                .and_then(|datetime| datetime.format(format))
                .expect("Failed to format DateTime")
                .to_string())
        }
    }

    //---------------------------------------
    // Satisfier associated functions
    //---------------------------------------
    pub fn has_local_satisfier(search_term: &str) -> bool {
        Self::with_alpm_handle(|handle| {
            handle.borrow().as_ref()
                .and_then(|handle| handle.localdb().pkgs().find_satisfier(search_term))
                .is_some()
        })
    }

    pub fn find_satisfier(search_term: &str, model: &gio::ListStore) -> Option<Self> {
        Self::with_alpm_handle(|handle| {
            let handle = handle.borrow();
            let handle = handle.as_ref()?;

            let db_pkg = handle.localdb().pkgs().find_satisfier(search_term)
                .or_else(|| handle.syncdbs().find_satisfier(search_term))?;

            model.iter::<Self>()
                .flatten()
                .find(|pkg| pkg.name() == db_pkg.name())
        })
    }
}
