use std::cell::{RefCell, OnceCell};
use std::rc::Rc;
use std::cmp::Ordering;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use alpm_utils::DbListExt;
use regex::Regex;
use size::Size;
use rayon::prelude::*;
use tokio::sync::OnceCell as TokioOnceCell;

use crate::window::{PACMAN_CONFIG, PACMAN_LOG, PACMAN_CACHE, PKGS, INSTALLED_PKGS, INSTALLED_PKG_NAMES};
use crate::pkg_data::{PkgData, PkgFlags, PkgValidation};

//------------------------------------------------------------------------------
// GLOBAL VARIABLES
//------------------------------------------------------------------------------
thread_local! {
    pub static ALPM_HANDLE: RefCell<Option<Rc<alpm::Alpm>>> = const { RefCell::new(None) };
}

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
    fn new(base64_sig: Option<&str>, sha256sum: Option<&str>, md5sum: Option<&str>) -> Self {
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
        // Alpm handle
        pub(super) handle: OnceCell<Rc<alpm::Alpm>>,

        // Read-write properties
        #[property(get, set, nullable)]
        update_version: RefCell<Option<String>>,

        // Read-only properties with getter
        #[property(name = "flags", get = Self::flags, type = PkgFlags)]
        #[property(name = "version", get = Self::version, type = String)]
        #[property(name = "show-version-icon", get = Self::show_version_icon, type = bool)]
        #[property(name = "status", get = Self::status, type = String)]
        #[property(name = "status-icon", get = Self::status_icon, type = String)]
        #[property(name = "status-icon-symbolic", get = Self::status_icon_symbolic, type = String)]
        #[property(name = "show-status-icon", get = Self::show_status_icon, type = bool)]
        #[property(name = "install-size-string", get = Self::install_size_string, type = String)]
        #[property(name = "show-groups-icon", get = Self::show_groups_icon, type = bool)]

        // Read-only properties from data fields
        #[property(name = "name", get, type = String, member = name)]
        #[property(name = "repository", get, type = String, member = repository)]
        #[property(name = "install-size", get, type = i64, member = install_size)]
        #[property(name = "groups", get, type = String, member = groups)]
        pub(super) data: OnceCell<PkgData>,

        // Read only fields
        pub(super) package_url: OnceCell<String>,
        pub(super) out_of_date_string: OnceCell<String>,
        pub(super) install_date_string: OnceCell<String>,
        pub(super) build_date_string: OnceCell<String>,
        pub(super) download_size_string: OnceCell<String>,

        pub(super) required_by: OnceCell<Vec<String>>,
        pub(super) optional_for: OnceCell<Vec<String>>,

        pub(super) files: OnceCell<Vec<String>>,
        pub(super) backup: OnceCell<Vec<PkgBackup>>,
        pub(super) hashes: OnceCell<PkgHashes>,

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
                .map_or_else(|| flags, |_| flags | PkgFlags::UPDATES)
        }

        fn version(&self) -> String {
            let version = &self.data.get().unwrap().version;

            self.update_version.borrow().as_ref()
                .map_or_else(|| version.to_string(), |update_version| {
                    version.to_string() + " \u{2192} " + update_version
                })
        }

        fn show_version_icon(&self) -> bool {
            self.flags().intersects(PkgFlags::UPDATES)
        }

        fn status(&self) -> String {
            match self.data.get().unwrap().flags {
                PkgFlags::EXPLICIT => "explicit",
                PkgFlags::DEPENDENCY => "dependency",
                PkgFlags::OPTIONAL => "optional",
                PkgFlags::ORPHAN => "orphan",
                _ => "not installed"
            }.to_owned()
        }

        fn status_icon(&self) -> String {
            match self.data.get().unwrap().flags {
                PkgFlags::EXPLICIT => "pkg-explicit",
                PkgFlags::DEPENDENCY => "pkg-dependency",
                PkgFlags::OPTIONAL => "pkg-optional",
                PkgFlags::ORPHAN => "pkg-orphan",
                _ => ""
            }.to_owned()
        }

        fn status_icon_symbolic(&self) -> String {
            match self.data.get().unwrap().flags {
                PkgFlags::EXPLICIT => "status-explicit-symbolic",
                PkgFlags::DEPENDENCY => "status-dependency-symbolic",
                PkgFlags::OPTIONAL => "status-optional-symbolic",
                PkgFlags::ORPHAN => "status-orphan-symbolic",
                _ => ""
            }.to_owned()
        }

        fn show_status_icon(&self) -> bool {
            self.data.get().unwrap().flags.intersects(PkgFlags::INSTALLED)
        }

        fn install_size_string(&self) -> String {
            Size::from_bytes(self.data.get().unwrap().install_size).to_string()
        }

        fn show_groups_icon(&self) -> bool {
            !self.data.get().unwrap().groups.is_empty()
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
    pub fn new(data: PkgData, handle: Option<Rc<alpm::Alpm>>) -> Self {
        let pkg: Self = glib::Object::builder().build();

        let imp = pkg.imp();

        imp.data.set(data).unwrap();

        if let Some(handle) = handle {
            imp.handle.set(handle).unwrap();
        }

        pkg.connect_update_version_notify(|pkg| {
            pkg.notify_flags();
            pkg.notify_version();
            pkg.notify_show_version_icon();
        });

        pkg
    }

    //---------------------------------------
    // Public data field getters
    //---------------------------------------
    pub fn description(&self) -> &str {
        &self.imp().data.get().unwrap().description
    }

    pub fn popularity(&self) -> &str {
        &self.imp().data.get().unwrap().popularity
    }

    pub fn out_of_date(&self) -> i64 {
        self.imp().data.get().unwrap().out_of_date
    }

    pub fn out_of_date_string(&self) -> &str {
        self.imp().out_of_date_string.get_or_init(|| {
            Self::date_to_string(self.imp().data.get().unwrap().out_of_date, "%d %B %Y %H:%M")
        })
    }

    pub fn package_url(&self) -> &str {
        self.imp().package_url.get_or_init(|| {
            let default_repos = ["core", "extra", "multilib"];

            let data = self.imp().data.get().unwrap();

            let repo = &data.repository;

            if default_repos.contains(&repo.as_str()) {
                format!("https://www.archlinux.org/packages/{repo}/{arch}/{name}",
                    arch=data.architecture,
                    name=data.name
                )
            } else if repo == "aur" {
                format!("https://aur.archlinux.org/packages/{name}",
                    name=data.name
                )
            } else {
                String::new()
            }
        })
    }

    pub fn url(&self) -> &str {
        &self.imp().data.get().unwrap().url
    }

    pub fn licenses(&self) -> &str {
        &self.imp().data.get().unwrap().licenses
    }

    pub fn depends(&self) -> &[String] {
        &self.imp().data.get().unwrap().depends
    }

    pub fn optdepends(&self) -> &[String] {
        &self.imp().data.get().unwrap().optdepends
    }

    pub fn makedepends(&self) -> &[String] {
        &self.imp().data.get().unwrap().makedepends
    }

    pub fn provides(&self) -> &[String] {
        &self.imp().data.get().unwrap().provides
    }

    pub fn conflicts(&self) -> &[String] {
        &self.imp().data.get().unwrap().conflicts
    }

    pub fn replaces(&self) -> &[String] {
        &self.imp().data.get().unwrap().replaces
    }

    pub fn architecture(&self) -> &str {
        &self.imp().data.get().unwrap().architecture
    }

    pub fn packager(&self) -> &str {
        &self.imp().data.get().unwrap().packager
    }

    pub fn install_date(&self) -> i64 {
        self.imp().data.get().unwrap().install_date
    }

    pub fn install_date_string(&self) -> &str {
        self.imp().install_date_string.get_or_init(|| {
            Self::date_to_string(self.imp().data.get().unwrap().install_date, "%d %B %Y %H:%M")
        })
    }

    pub fn build_date(&self) -> i64 {
        self.imp().data.get().unwrap().build_date
    }

    pub fn build_date_string(&self) -> &str {
        self.imp().build_date_string.get_or_init(|| {
            Self::date_to_string(self.imp().data.get().unwrap().build_date, "%d %B %Y %H:%M")
        })
    }

    pub fn download_size(&self) -> i64 {
        self.imp().data.get().unwrap().download_size
    }

    pub fn download_size_string(&self) -> &str {
        self.imp().download_size_string.get_or_init(|| {
            Size::from_bytes(self.imp().data.get().unwrap().download_size).to_string()
        })
    }

    pub fn has_script(&self) -> &str {
        &self.imp().data.get().unwrap().has_script
    }

    pub fn validation(&self) -> PkgValidation {
        self.imp().data.get().unwrap().validation
    }

    //---------------------------------------
    // Get alpm package helper functions
    //---------------------------------------
    fn pkg(&self) -> Option<&alpm::Package> {
        let imp = self.imp();

        let handle = imp.handle.get()?;
        let data = imp.data.get().unwrap();

        if data.flags.intersects(PkgFlags::INSTALLED) {
            handle.localdb().pkg(data.name.as_str()).ok()
        } else {
            handle.syncdbs().pkg(data.name.as_str()).ok()
        }
    }

    fn sync_pkg(&self) -> Option<&alpm::Package> {
        let imp = self.imp();

        let handle = imp.handle.get()?;
        let data = imp.data.get().unwrap();

        handle.syncdbs().pkg(data.name.as_str()).ok()
    }

    //---------------------------------------
    // Public getters from alpm package
    //---------------------------------------
    pub fn required_by(&self) -> &[String] {
        self.imp().required_by.get_or_init(|| {
            self.pkg()
                .map(|pkg| {
                    let mut required_by: Vec<String> = pkg.required_by().into_iter()
                        .collect();

                    required_by.par_sort_unstable();

                    required_by
                })
                .unwrap_or_default()
        })
    }

    pub fn optional_for(&self) -> &[String] {
        self.imp().optional_for.get_or_init(|| {
            self.pkg()
                .map(|pkg| {
                    let mut optional_for: Vec<String> = pkg.optional_for().into_iter()
                        .collect();

                    optional_for.par_sort_unstable();

                    optional_for
                })
                .unwrap_or_default()
        })
    }

    pub fn files(&self) -> &[String] {
        let imp = self.imp();

        imp.files.get_or_init(|| {
            self.pkg()
                .map(|pkg| {
                    let root_dir = &PACMAN_CONFIG.get().unwrap().root_dir;

                    let mut files: Vec<String> = pkg.files().files().iter()
                        .map(|file| root_dir.to_string() + file.name())
                        .collect();

                    files.par_sort_unstable();

                    files
                })
                .unwrap_or_default()
        })
    }

    pub fn backup(&self) -> &[PkgBackup] {
        let imp = self.imp();

        imp.backup.get_or_init(|| {
            self.pkg()
                .map(|pkg| {
                    let root_dir = &PACMAN_CONFIG.get().unwrap().root_dir;
                    let pkg_name = self.name();

                    let mut backup: Vec<PkgBackup> = pkg.backup().iter()
                        .map(|backup| {
                            PkgBackup::new(&(root_dir.to_string() + backup.name()), backup.hash(), &pkg_name)
                        })
                        .collect();

                    backup.par_sort_unstable_by(|backup_a, backup_b| {
                        backup_a.filename.partial_cmp(&backup_b.filename).unwrap_or(Ordering::Equal)
                    });

                    backup
                })
                .unwrap_or_default()
        })
    }

    pub fn hashes(&self) -> &PkgHashes {
        let imp = self.imp();

        imp.hashes.get_or_init(|| {
            self.sync_pkg()
                .map(|pkg| PkgHashes::new(pkg.base64_sig(), pkg.sha256sum(), pkg.md5sum()))
                .unwrap_or_default()
        })
    }

    //---------------------------------------
    // Public async getters from alpm package
    //---------------------------------------
    pub async fn log_async(&self) -> &[String] {
        let imp = self.imp();

        imp.log.get_or_init(async || {
            let pkg_name = self.name();

            let expr = Regex::new(&format!(r"\[(.+?)T(.+?)\+.+?\] \[ALPM\] (installed|removed|upgraded|downgraded) ({name}) (.+)", name=regex::escape(&pkg_name)))
                .expect("Failed to compile Regex");

            gio::spawn_blocking(move || {
                let pacman_log = PACMAN_LOG.lock().unwrap();

                pacman_log.as_ref().map_or(vec![], |log| {
                    log.lines().rev()
                        .filter(|&line| line.contains(&pkg_name) && expr.is_match(line))
                        .map(|line| expr.replace(line, "[$1  $2]  $3 $4 $5").into_owned())
                        .collect::<Vec<String>>()
                })
            })
            .await
            .expect("Failed to complete task")
        })
        .await
    }

    pub async fn cache_async(&self) -> &[String] {
        let imp = self.imp();

        imp.cache.get_or_init(async || {
            let pkg_name = self.name();

            gio::spawn_blocking(move || {
                let pacman_cache = PACMAN_CACHE.lock().unwrap();

                pacman_cache.iter()
                    .filter_map(|path| {
                        path.file_name()
                            .and_then(|filename| filename.to_str())
                            .filter(|&filename| filename.ends_with(".pkg.tar.zst"))
                            .filter(|&filename| 
                                filename.rsplitn(4, '-').last()
                                    .is_some_and(|name| name == pkg_name)
                            )
                            .map(ToOwned::to_owned)
                    })
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
    pub fn date_to_string(date: i64, format: &str) -> String {
        if date == 0 {
            String::new()
        } else {
            glib::DateTime::from_unix_local(date)
                .and_then(|datetime| datetime.format(format))
                .expect("Failed to format DateTime")
                .to_string()
        }
    }

    //---------------------------------------
    // Satisfier associated functions
    //---------------------------------------
    pub fn has_local_satisfier(search_term: &str) -> Option<bool> {
        ALPM_HANDLE.with_borrow(|alpm_handle| {
            let handle = alpm_handle.as_ref()?;

            handle.localdb().pkgs().find_satisfier(search_term)
                .map(|local_pkg| {
                    INSTALLED_PKG_NAMES.with_borrow(|installed_pkg_names| {
                        installed_pkg_names.contains(local_pkg.name())
                    })
                })
        })
    }

    pub fn find_satisfier(search_term: &str) -> Option<Self> {
        ALPM_HANDLE.with_borrow(|alpm_handle| {
            let handle = alpm_handle.as_ref()?;

            if let Some(local_pkg) = handle.localdb().pkgs().find_satisfier(search_term) {
                return INSTALLED_PKGS.with_borrow(|installed_pkgs| {
                    installed_pkgs.iter()
                        .find(|&pkg| pkg.name() == local_pkg.name())
                        .cloned()
                })
            }

            if let Some(sync_pkg) = handle.syncdbs().find_satisfier(search_term) {
                return PKGS.with_borrow(|pkgs| {
                    pkgs.iter()
                        .find(|&pkg| pkg.name() == sync_pkg.name())
                        .cloned()
                })
            }

            None
        })
    }
}
