use std::cell::{RefCell, OnceCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::cmp::Ordering;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use alpm_utils::DbListExt;
use itertools::Itertools;
use regex::Regex;
use glob::glob;

use crate::window::{PACMAN_LOG, PACMAN_CONFIG};
use crate::utils::{date_to_string, size_to_string};

//------------------------------------------------------------------------------
// GLOBAL VARIABLES
//------------------------------------------------------------------------------
thread_local! {
    pub static ALPM_HANDLE: RefCell<Option<Rc<alpm::Alpm>>> = const {RefCell::new(None)};
    pub static AUR_NAMES: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    pub static PKGS: RefCell<Vec<PkgObject>> = const {RefCell::new(vec![])};
    pub static INSTALLED_PKGS: RefCell<Vec<PkgObject>> = const {RefCell::new(vec![])};
    pub static INSTALLED_PKG_NAMES: RefCell<Arc<HashSet<String>>> = RefCell::new(Arc::new(HashSet::new()));
}

//------------------------------------------------------------------------------
// GLOBAL: Helper functions
//------------------------------------------------------------------------------
fn alpm_list_to_string(list: &alpm::AlpmList<&str>) -> String {
    list.iter()
        .sorted_unstable()
        .join(" | ")
}

fn alpm_deplist_to_vec(list: &alpm::AlpmList<&alpm::Dep>) -> Vec<String> {
    list.iter()
        .map(|dep| dep.to_string())
        .sorted_unstable()
        .collect()
}

fn aur_vec_to_string(vec: &[String]) -> String {
    vec.iter()
        .sorted_unstable()
        .join(" | ")
}

fn aur_sorted_vec(vec: &[String]) -> Vec<String> {
    vec.iter()
        .map(String::from)
        .sorted_unstable()
        .collect()
}

//------------------------------------------------------------------------------
// ENUM: PkgData
//------------------------------------------------------------------------------
pub enum PkgData<'a> {
    Handle(Rc<alpm::Alpm>, &'a alpm::Package),
    AurPkg(raur::ArcPackage),
}

//------------------------------------------------------------------------------
// ENUM: PkgInternal
//------------------------------------------------------------------------------
enum PkgInternal<'a> {
    Pacman(&'a alpm::Package),
    Aur(&'a raur::ArcPackage),
    None
}

//------------------------------------------------------------------------------
// FLAGS: PkgFlags
//------------------------------------------------------------------------------
#[glib::flags(name = "PkgFlags")]
pub enum PkgFlags {
    ALL        = Self::INSTALLED.bits() | Self::NONE.bits(),
    INSTALLED  = Self::EXPLICIT.bits() | Self::DEPENDENCY.bits() | Self::OPTIONAL.bits() | Self::ORPHAN.bits(),
    EXPLICIT   = 0b00000001,
    DEPENDENCY = 0b00000010,
    OPTIONAL   = 0b00000100,
    ORPHAN     = 0b00001000,
    NONE       = 0b00010000,
    UPDATES    = 0b00100000,
}

impl Default for PkgFlags {
    fn default() -> Self {
        PkgFlags::empty()
    }
}

//------------------------------------------------------------------------------
// STRUCT: PkgBackup
//------------------------------------------------------------------------------
#[derive(Default, Debug, Clone)]
pub struct PkgBackup {
    filename: String,
    hash: String,
    file_hash: Option<String>,
    package: String
}

impl PkgBackup {
    fn new(filename: &str, hash: &str, package: &str) -> Self {
        let file_hash = alpm::compute_md5sum(filename).ok();

        Self {
            filename: filename.to_string(),
            hash: hash.to_string(),
            file_hash,
            package: package.to_string()
        }
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn file_hash(&self) -> Option<&str> {
        self.file_hash.as_deref()
    }

    pub fn package(&self) -> &str {
        &self.package
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

        // AUR package
        pub(super) aur_pkg: OnceCell<raur::ArcPackage>,

        // Read-write properties, construct only
        #[property(get, set, construct_only)]
        name: RefCell<String>,
        #[property(get, set, construct_only)]
        repository: RefCell<String>,

        // Read-write properties
        #[property(get, set, nullable)]
        update_version: RefCell<Option<String>>,

        // Read only properties
        #[property(get = Self::flags)]
        flags: OnceCell<PkgFlags>,
        #[property(get = Self::version)]
        version: OnceCell<String>,
        #[property(get = Self::status)]
        _status: OnceCell<String>,
        #[property(get = Self::status_icon)]
        _status_icon: OnceCell<String>,
        #[property(get = Self::status_icon_symbolic)]
        _status_icon_symbolic: OnceCell<String>,
        #[property(get = Self::groups)]
        groups: OnceCell<String>,
        #[property(get = Self::install_size)]
        install_size: OnceCell<i64>,
        #[property(get = Self::install_size_string)]
        _install_size_string: OnceCell<String>,

        // Read only fields
        pub(super) description: OnceCell<String>,
        pub(super) url: OnceCell<String>,
        pub(super) depends: OnceCell<Vec<String>>,
        pub(super) optdepends: OnceCell<Vec<String>>,
        pub(super) makedepends: OnceCell<Vec<String>>,
        pub(super) required_by: OnceCell<Vec<String>>,
        pub(super) optional_for: OnceCell<Vec<String>>,
        pub(super) provides: OnceCell<Vec<String>>,
        pub(super) conflicts: OnceCell<Vec<String>>,
        pub(super) replaces: OnceCell<Vec<String>>,
        pub(super) licenses: OnceCell<String>,
        pub(super) architecture: OnceCell<String>,
        pub(super) packager: OnceCell<String>,
        pub(super) build_date: OnceCell<i64>,
        pub(super) install_date: OnceCell<i64>,
        pub(super) download_size: OnceCell<i64>,
        pub(super) has_script: OnceCell<bool>,
        pub(super) sha256sum: OnceCell<String>,

        pub(super) files: OnceCell<Vec<String>>,
        pub(super) log: OnceCell<Vec<String>>,
        pub(super) cache: OnceCell<Vec<String>>,
        pub(super) backup: OnceCell<Vec<PkgBackup>>,
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
        // Get alpm package helper functions
        //---------------------------------------
        pub(super) fn sync_pkg(&self) -> Option<&alpm::Package> {
            self.handle.get()
                .and_then(|handle| handle.syncdbs().pkg(self.obj().name()).ok())
        }

        pub(super) fn local_pkg(&self) -> Option<&alpm::Package> {
            self.handle.get()
                .and_then(|handle| handle.localdb().pkg(self.obj().name()).ok())
        }

        pub(super) fn pkg(&self) -> PkgInternal {
            if self.flags().intersects(PkgFlags::INSTALLED) {
                self.local_pkg()
            } else {
                self.sync_pkg()
            }
            .map(PkgInternal::Pacman)
            .unwrap_or({
                self.aur_pkg.get()
                    .map(PkgInternal::Aur)
                    .unwrap_or(PkgInternal::None)
            })
        }

        //---------------------------------------
        // Read-only property getters
        //---------------------------------------
        fn flags(&self) -> PkgFlags {
            let flags = self.flags.get_or_init(|| {
                self.local_pkg()
                    .map(|pkg| {
                        if pkg.reason() == alpm::PackageReason::Explicit {
                            PkgFlags::EXPLICIT
                        } else {
                            self.required_by.set(pkg.required_by().into_iter()
                                .sorted_unstable()
                                .collect()
                            )
                            .unwrap();

                            if !pkg.required_by().is_empty() {
                                PkgFlags::DEPENDENCY
                            } else {
                                self.optional_for.set(pkg.optional_for().into_iter()
                                    .sorted_unstable()
                                    .collect()
                                )
                                .unwrap();

                                if !pkg.optional_for().is_empty() {
                                    PkgFlags::OPTIONAL
                                } else {
                                    PkgFlags::ORPHAN
                                }
                            }
                        }
                    })
                    .unwrap_or(PkgFlags::NONE)
            });

            if self.obj().update_version().is_some() {
                *flags | PkgFlags::UPDATES
            } else {
                *flags
            }
        }

        fn version(&self) -> String {
            let version = self.version.get_or_init(|| {
                match self.pkg() {
                    PkgInternal::Pacman(pkg) => { pkg.version().as_str() },
                    PkgInternal::Aur(pkg) => { &pkg.version },
                    PkgInternal::None => { "" }
                }
                .to_string()
            });

            if let Some(update_version) = self.obj().update_version() {
                format!("{version} \u{2192} {update_version}")
            } else {
                version.to_string()
            }
        }

        fn status(&self) -> String {
            let flags = self.flags() & !PkgFlags::UPDATES;

            match flags {
                PkgFlags::EXPLICIT => "explicit",
                PkgFlags::DEPENDENCY => "dependency",
                PkgFlags::OPTIONAL => "optional",
                PkgFlags::ORPHAN => "orphan",
                _ => "not installed"
            }
            .to_string()
        }

        fn status_icon(&self) -> String {
            let flags = self.flags() & !PkgFlags::UPDATES;

            match flags {
                PkgFlags::EXPLICIT => "pkg-explicit",
                PkgFlags::DEPENDENCY => "pkg-dependency",
                PkgFlags::OPTIONAL => "pkg-optional",
                PkgFlags::ORPHAN => "pkg-orphan",
                _ => ""
            }
            .to_string()
        }

        fn status_icon_symbolic(&self) -> String {
            let flags = self.flags() & !PkgFlags::UPDATES;

            match flags {
                PkgFlags::EXPLICIT => "status-explicit-symbolic",
                PkgFlags::DEPENDENCY => "status-dependency-symbolic",
                PkgFlags::OPTIONAL => "status-optional-symbolic",
                PkgFlags::ORPHAN => "status-orphan-symbolic",
                _ => ""
            }
            .to_string()
        }

        fn groups(&self) -> String {
            self.groups.get_or_init(|| {
                match self.pkg() {
                    PkgInternal::Pacman(pkg) => { alpm_list_to_string(&pkg.groups()) },
                    PkgInternal::Aur(pkg) => { aur_vec_to_string(&pkg.groups) },
                    PkgInternal::None => { String::from("") }
                }
            })
            .to_string()
        }

        fn install_size(&self) -> i64 {
            *self.install_size.get_or_init(|| {
                match self.pkg() {
                    PkgInternal::Pacman(pkg) => { pkg.isize() },
                    _ => { 0 },
                }
            })
        }

        fn install_size_string(&self) -> String {
            size_to_string(self.install_size(), 1)
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
    pub fn new(name: &str, data: PkgData) -> Self {
        let repo = match data {
            PkgData::Handle(_, pkg) => {
                let mut repo = pkg.db().map(|db| db.name()).unwrap_or_default();

                if repo == "local" {
                    AUR_NAMES.with_borrow(|aur_names| {
                        if aur_names.contains(pkg.name()) {
                            repo = "aur"
                        }
                    });
                }

                repo
            },
            PkgData::AurPkg(_) => {
                "aur"
            }
        };

        let pkg: Self = glib::Object::builder()
            .property("name", name)
            .property("repository", repo)
            .build();

        let imp = pkg.imp();

        match data {
            PkgData::Handle(handle, _) => { imp.handle.set(handle).unwrap() },
            PkgData::AurPkg(pkg) => { imp.aur_pkg.set(pkg).unwrap(); }
        }

        pkg.connect_update_version_notify(|pkg| {
            pkg.notify_flags();
            pkg.notify_version();
        });

        pkg
    }

    //---------------------------------------
    // Public internal field getters
    //---------------------------------------
    pub fn description(&self) -> &str {
        let imp = self.imp();

        imp.description.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { pkg.desc() },
                PkgInternal::Aur(pkg) => { pkg.description.as_deref() },
                PkgInternal::None => { None }
            }
            .unwrap_or_default()
            .to_string()
        })
    }

    pub fn url(&self) -> &str {
        let imp = self.imp();

        imp.url.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { pkg.url() },
                PkgInternal::Aur(pkg) => { pkg.url.as_deref() },
                PkgInternal::None => { None }
            }
            .unwrap_or_default()
            .to_string()
        })
    }

    pub fn depends(&self) -> &[String] {
        let imp = self.imp();

        imp.depends.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { alpm_deplist_to_vec(&pkg.depends()) },
                PkgInternal::Aur(pkg) => { aur_sorted_vec(&pkg.depends) },
                PkgInternal::None => { vec![] }
            }
        })
    }

    pub fn optdepends(&self) -> &[String] {
        let imp = self.imp();

        imp.optdepends.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { alpm_deplist_to_vec(&pkg.optdepends()) },
                PkgInternal::Aur(pkg) => { aur_sorted_vec(&pkg.opt_depends) },
                PkgInternal::None => { vec![] }
            }
        })
    }

    pub fn makedepends(&self) -> &[String] {
        let imp = self.imp();

        imp.makedepends.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Aur(pkg) => { aur_sorted_vec(&pkg.make_depends) },
                _ => { vec![] }
            }
        })
    }

    pub fn required_by(&self) -> &[String] {
        let imp = self.imp();

        imp.required_by.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { pkg.required_by().into_iter()
                    .sorted_unstable()
                    .collect()
                },
                _ => { vec![] }
            }
        })
    }

    pub fn optional_for(&self) -> &[String] {
        let imp = self.imp();

        imp.optional_for.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { pkg.optional_for().into_iter()
                    .sorted_unstable()
                    .collect()
                },
                _ => { vec![] }
            }
        })
    }

    pub fn provides(&self) -> &[String] {
        let imp = self.imp();

        imp.provides.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { alpm_deplist_to_vec(&pkg.provides()) },
                PkgInternal::Aur(pkg) => { aur_sorted_vec(&pkg.provides) },
                PkgInternal::None => { vec![] }
            }
        })
    }

    pub fn conflicts(&self) -> &[String] {
        let imp = self.imp();

        imp.conflicts.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { alpm_deplist_to_vec(&pkg.conflicts()) },
                PkgInternal::Aur(pkg) => { aur_sorted_vec(&pkg.conflicts) },
                PkgInternal::None => { vec![] }
            }
        })
    }

    pub fn replaces(&self) -> &[String] {
        let imp = self.imp();

        imp.replaces.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { alpm_deplist_to_vec(&pkg.replaces()) },
                PkgInternal::Aur(pkg) => { aur_sorted_vec(&pkg.replaces) },
                PkgInternal::None => { vec![] }
            }
        })
    }

    pub fn licenses(&self) -> &str {
        let imp = self.imp();

        imp.licenses.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { alpm_list_to_string(&pkg.licenses()) },
                PkgInternal::Aur(pkg) => { aur_vec_to_string(&pkg.license) },
                PkgInternal::None => { String::from("") }
            }
        })
    }

    pub fn architecture(&self) -> &str {
        let imp = self.imp();

        imp.architecture.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { pkg.arch() },
                _ => { None },
            }
            .unwrap_or_default()
            .to_string()
        })
    }

    pub fn packager(&self) -> &str {
        let imp = self.imp();

        imp.packager.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { pkg.packager() },
                PkgInternal::Aur(pkg) => { pkg.maintainer.as_deref() },
                PkgInternal::None => { None }
            }
            .unwrap_or("Unknown Packager")
            .to_string()
        })
    }

    pub fn build_date(&self) -> i64 {
        let imp = self.imp();

        *imp.build_date.get_or_init(|| {
            imp.sync_pkg()
                .map(|pkg| pkg.build_date())
                .unwrap_or_default()
        })
    }

    pub fn install_date(&self) -> i64 {
        let imp = self.imp();

        *imp.install_date.get_or_init(|| {
            imp.local_pkg()
                .and_then(|pkg| pkg.install_date())
                .unwrap_or_default()
        })
    }

    pub fn download_size(&self) -> i64 {
        let imp = self.imp();

        *imp.download_size.get_or_init(|| {
            imp.sync_pkg()
                .map(|pkg| pkg.download_size())
                .unwrap_or_default()
        })
    }

    pub fn has_script(&self) -> bool {
        let imp = self.imp();

        *imp.has_script.get_or_init(|| {
            match imp.pkg() {
                PkgInternal::Pacman(pkg) => { pkg.has_scriptlet() },
                _ => { false },
            }
        })
    }

    pub fn sha256sum(&self) -> &str {
        let imp = self.imp();

        imp.sha256sum.get_or_init(|| {
            imp.sync_pkg()
                .and_then(|pkg| pkg.sha256sum())
                .unwrap_or_default()
                .to_string()
        })
    }

    pub fn files(&self) -> &[String] {
        let imp = self.imp();

        imp.files.get_or_init(|| {
            let pacman_config = PACMAN_CONFIG.get().unwrap();

            imp.local_pkg()
                .map(|pkg| {
                    pkg.files().files().iter()
                        .map(|file| format!("{}{}", pacman_config.root_dir, file.name()))
                        .sorted_unstable()
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    pub fn log(&self) -> &[String] {
        PACMAN_LOG.with_borrow(|pacman_log| {
            self.imp().log.get_or_init(|| {
                let mut log_lines: Vec<String> = vec![];

                if !pacman_log.is_empty() {
                    let expr = Regex::new(&format!(r"\[(.+?)T(.+?)\+.+?\] \[ALPM\] (installed|removed|upgraded|downgraded) ({name}) (.+)", name=regex::escape(&self.name())))
                        .expect("Regex error");

                    log_lines.extend(pacman_log.lines().rev()
                        .filter_map(|s| {
                            if expr.is_match(s) {
                                Some(expr.replace(s, "[$1  $2] : $3 $4 $5").into_owned())
                            } else {
                                None
                            }
                        })
                    )
                }

                log_lines
            })
        })
    }

    pub fn cache(&self) -> &[String] {
        INSTALLED_PKG_NAMES.with_borrow(|installed_pkg_names| {
            self.imp().cache.get_or_init(|| {
                let pkg_name = self.name();

                // Get cache blacklist package names
                let cache_blacklist: Vec<&String> = installed_pkg_names.iter()
                    .filter(|&name| name.starts_with(&pkg_name) && name != &pkg_name)
                    .collect();

                let pacman_config = PACMAN_CONFIG.get().unwrap();

                pacman_config.cache_dir.iter()
                    .flat_map(|dir| {
                        glob(&format!("{dir}{pkg_name}*.pkg.tar.zst"))
                            .expect("Glob pattern error")
                            .flatten()
                            .filter_map(|path| {
                                let cache_file = path.display().to_string();

                                // Exclude cache files that include blacklist package names
                                if cache_blacklist.iter().any(|&s| cache_file.contains(s)) {
                                    None
                                } else {
                                    Some(cache_file)
                                }
                            })
                    })
                    .collect::<Vec<String>>()
            })
        })
    }

    pub fn backup(&self) -> &[PkgBackup] {
        let imp = self.imp();

        imp.backup.get_or_init(|| {
            let pacman_config = PACMAN_CONFIG.get().unwrap();

            imp.local_pkg()
                .map(|pkg| {
                    pkg.backup().iter()
                        .map(|backup|
                            PkgBackup::new(&format!("{}{}", pacman_config.root_dir, backup.name()), backup.hash(), pkg.name())
                        )
                        .sorted_unstable_by(|backup_a, backup_b|
                            backup_a.filename.partial_cmp(&backup_b.filename).unwrap_or(Ordering::Equal)
                        )
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    //---------------------------------------
    // Other public functions
    //---------------------------------------
    pub fn package_url(&self) -> String {
        let default_repos = ["core", "extra", "multilib"];

        let repo = self.repository();

        if default_repos.contains(&repo.as_str()) {
            format!("https://www.archlinux.org/packages/{repo}/{arch}/{name}",
                arch=self.architecture(),
                name=self.name()
            )
        } else if repo == "aur" {
            format!("https://aur.archlinux.org/packages/{name}",
                name=self.name()
            )
        } else {
            String::from("")
        }
    }

    pub fn build_date_string(&self) -> String {
        date_to_string(self.build_date(), "%d %B %Y %H:%M")
    }

    pub fn install_date_string(&self) -> String {
        date_to_string(self.install_date(), "%d %B %Y %H:%M")
    }

    pub fn download_size_string(&self) -> String {
        size_to_string(self.download_size(), 1)
    }

    //---------------------------------------
    // Public associated functions
    //---------------------------------------
    pub fn find_satisfier(search_term: &str, include_sync: bool) -> Option<PkgObject> {
        ALPM_HANDLE.with_borrow(|alpm_handle| {
            alpm_handle.as_ref()
                .and_then(|handle|
                    handle.localdb().pkgs().find_satisfier(search_term)
                    .or_else(||
                        if include_sync {
                            handle.syncdbs().find_satisfier(search_term)
                        } else {
                            None
                        }
                    )
                    .map(|pkg| PkgObject::new(pkg.name(), PkgData::Handle(Rc::clone(handle), pkg)))
                )
        })
    }
}
