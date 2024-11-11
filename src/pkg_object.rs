use std::cell::{RefCell, OnceCell};
use std::rc::Rc;
use std::cmp::Ordering;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use alpm_utils::DbListExt;
use itertools::Itertools;

use crate::window::PACMAN_CONFIG;
use crate::utils::{date_to_string, size_to_string};

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
        .map(|s| s.to_string())
        .sorted_unstable()
        .collect()
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
// ENUM: PkgData
//------------------------------------------------------------------------------
pub enum PkgData {
    Handle(Rc<alpm::Alpm>),
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
        pub(super) backup: OnceCell<Vec<(String, String, String)>>,
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
        pub(super) fn sync_pkg(&self) -> Result<&alpm::Package, alpm::Error> {
            self.handle.get()
                .ok_or(alpm::Error::HandleNull)
                .and_then(|handle| handle.syncdbs().pkg(self.obj().name()))
        }

        pub(super) fn local_pkg(&self) -> Result<&alpm::Package, alpm::Error> {
            self.handle.get()
                .ok_or(alpm::Error::HandleNull)
                .and_then(|handle| handle.localdb().pkg(self.obj().name()))
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
    pub fn new(name: &str, repo: &str, data: PkgData) -> Self {
        let pkg: Self = glib::Object::builder()
            .property("name", name)
            .property("repository", repo)
            .build();

        let imp = pkg.imp();

        match data {
            PkgData::Handle(handle) => { imp.handle.set(handle).unwrap() },
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
            imp.local_pkg().ok()
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
            imp.sync_pkg().ok()
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

    pub fn backup(&self) -> &[(String, String, String)] {
        let imp = self.imp();

        imp.backup.get_or_init(|| {
            let pacman_config = PACMAN_CONFIG.get().unwrap();

            imp.local_pkg()
                .map(|pkg| {
                    pkg.backup().iter()
                        .map(|backup| {
                            let filename = format!("{}{}", pacman_config.root_dir, backup.name());

                            let file_hash = alpm::compute_md5sum(filename.as_str())
                                .unwrap_or_default();

                            (filename, backup.hash().to_string(), file_hash)
                        })
                        .sorted_unstable_by(|(a_file, _, _), (b_file, _, _)|
                            a_file.partial_cmp(b_file).unwrap_or(Ordering::Equal)
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
}
