use std::cell::{RefCell, OnceCell};
use std::rc::Rc;
use std::cmp::Ordering;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use alpm_utils::DbListExt;
use itertools::Itertools;
use regex::Regex;
use glob::glob;

use crate::window::{PACMAN_CONFIG, PACMAN_LOG, ALPM_HANDLE, PKGS, INSTALLED_PKGS, INSTALLED_PKG_NAMES};
use crate::pkg_data::{PkgFlags, PkgData};
use crate::utils::{date_to_string, size_to_string};

//------------------------------------------------------------------------------
// STRUCT: PkgBackup
//------------------------------------------------------------------------------
#[derive(Default, Debug, Clone)]
pub struct PkgBackup {
    filename: String,
    hash: String,
    package: String
}

impl PkgBackup {
    fn new(filename: &str, hash: &str, package: &str) -> Self {
        Self {
            filename: filename.to_string(),
            hash: hash.to_string(),
            package: package.to_string()
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

        // Read-only properties with custom getter
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
        pub(super) required_by: OnceCell<Vec<String>>,
        pub(super) optional_for: OnceCell<Vec<String>>,

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
        // Read-only property getters
        //---------------------------------------
        fn flags(&self) -> PkgFlags {
            let flags = self.data.get().unwrap().flags;

            self.obj().update_version()
                .map_or_else(|| flags, |_| flags | PkgFlags::UPDATES)
        }

        fn version(&self) -> String {
            let version = &self.data.get().unwrap().version;

            self.obj().update_version()
                .map_or_else(|| version.to_string(), |update_version| {
                    format!("{version} \u{2192} {update_version}")
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
            }
            .to_string()
        }

        fn status_icon(&self) -> String {
            match self.data.get().unwrap().flags {
                PkgFlags::EXPLICIT => "pkg-explicit",
                PkgFlags::DEPENDENCY => "pkg-dependency",
                PkgFlags::OPTIONAL => "pkg-optional",
                PkgFlags::ORPHAN => "pkg-orphan",
                _ => ""
            }
            .to_string()
        }

        fn status_icon_symbolic(&self) -> String {
            match self.data.get().unwrap().flags {
                PkgFlags::EXPLICIT => "status-explicit-symbolic",
                PkgFlags::DEPENDENCY => "status-dependency-symbolic",
                PkgFlags::OPTIONAL => "status-optional-symbolic",
                PkgFlags::ORPHAN => "status-orphan-symbolic",
                _ => ""
            }
            .to_string()
        }

        fn show_status_icon(&self) -> bool {
            self.data.get().unwrap().flags.intersects(PkgFlags::INSTALLED)
        }

        fn install_size_string(&self) -> String {
            size_to_string(self.data.get().unwrap().install_size, 1)
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
        let pkg: Self = glib::Object::builder()
            .build();

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
    // Get alpm package helper function
    //---------------------------------------
    pub(super) fn pkg(&self) -> Option<&alpm::Package> {
        let imp = self.imp();

        let handle = imp.handle.get();
        let data = imp.data.get().unwrap();

        if data.flags.intersects(PkgFlags::INSTALLED) {
            handle.and_then(|handle| handle.localdb().pkg(data.name.as_str()).ok())
        } else {
            handle.and_then(|handle| handle.syncdbs().pkg(data.name.as_str()).ok())
        }
    }

    //---------------------------------------
    // Public internal field getters/setters
    //---------------------------------------
    pub fn description(&self) -> &str {
        &self.imp().data.get().unwrap().description
    }

    pub fn package_url(&self) -> String {
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

    pub fn required_by(&self) -> &[String] {
        self.imp().required_by.get_or_init(|| {
            self.pkg()
                .map(|pkg| {
                    pkg.required_by().into_iter()
                        .sorted_unstable()
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default()
        })
    }

    pub fn optional_for(&self) -> &[String] {
        self.imp().optional_for.get_or_init(|| {
            self.pkg()
                .map(|pkg| {
                    pkg.optional_for().into_iter()
                        .sorted_unstable()
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default()
        })
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

    pub fn install_date_string(&self) -> String {
        date_to_string(self.imp().data.get().unwrap().install_date, "%d %B %Y %H:%M")
    }

    pub fn build_date(&self) -> i64 {
        self.imp().data.get().unwrap().build_date
    }

    pub fn build_date_string(&self) -> String {
        date_to_string(self.imp().data.get().unwrap().build_date, "%d %B %Y %H:%M")
    }

    pub fn download_size(&self) -> i64 {
        self.imp().data.get().unwrap().download_size
    }

    pub fn download_size_string(&self) -> String {
        size_to_string(self.imp().data.get().unwrap().download_size, 1)
    }

    pub fn has_script(&self) -> bool {
        self.imp().data.get().unwrap().has_script
    }

    pub fn sha256sum(&self) -> &str {
        &self.imp().data.get().unwrap().sha256sum
    }

    pub fn files(&self) -> &[String] {
        let imp = self.imp();

        imp.files.get_or_init(|| {
            let pacman_config = PACMAN_CONFIG.get().unwrap();

            self.pkg()
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
                pacman_log.as_ref().map_or(vec![], |log| {
                    let expr = Regex::new(&format!(r"\[(.+?)T(.+?)\+.+?\] \[ALPM\] (installed|removed|upgraded|downgraded) ({name}) (.+)", name=regex::escape(&self.name())))
                        .expect("Regex error");

                    log.lines().rev()
                        .filter_map(|s| {
                            if expr.is_match(s) {
                                Some(expr.replace(s, "[$1  $2] : $3 $4 $5").into_owned())
                            } else {
                                None
                            }
                        })
                        .collect()
                })
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

            self.pkg()
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
    // Public associated functions
    //---------------------------------------
    pub fn find_satisfier(search_term: &str, include_sync: bool) -> Option<Self> {
        ALPM_HANDLE.with_borrow(|alpm_handle| {
            alpm_handle.as_ref()
                .and_then(|handle| {
                    let mut pkg = handle.localdb().pkgs().find_satisfier(search_term)
                        .and_then(|local_pkg|
                            INSTALLED_PKGS.with_borrow(|installed_pkgs| {
                                installed_pkgs.iter().find(|&pkg| pkg.name() == local_pkg.name()).cloned()
                            })
                        );

                    if include_sync && pkg.is_none() {
                        pkg = handle.syncdbs().find_satisfier(search_term)
                            .and_then(|sync_pkg|
                                PKGS.with_borrow(|pkgs| {
                                    pkgs.iter().find(|&pkg| pkg.name() == sync_pkg.name()).cloned()
                                })
                            );
                    }

                    pkg
                })
        })
    }
}
