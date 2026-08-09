use std::cell::{RefCell, OnceCell};
use std::cmp::Ordering;

use gtk::{glib, gio};
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;
use glib::GString;

use alpm::{Alpm, Package};
use alpm_utils::DbListExt;
use size::Size;
use walkdir::WalkDir;

use crate::{
    utils::{Paths, Pacman, Paru, ListStoreFind},
    pkg_data::{PkgData, PkgFlags, PkgValidation}
};

//------------------------------------------------------------------------------
// STRUCT: PkgBackup
//------------------------------------------------------------------------------
#[derive(Debug)]
pub struct PkgBackup {
    pub path: String,
    pub hash: String
}

//------------------------------------------------------------------------------
// STRUCT: PkgHashes
//------------------------------------------------------------------------------
#[derive(Default, Debug)]
pub struct PkgHashes {
    pub base64_sig: Option<String>,
    pub sha256sum: Option<String>,
    pub md5sum: Option<String>
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

        // Read-only properties from data fields
        #[property(name = "name", get, type = String, member = name)]
        #[property(name = "version", get, type = String, member = version)]
        #[property(name = "repository", get, type = String, member = repository)]
        pub(super) data: OnceCell<PkgData>,

        // Read only fields
        pub(super) required_by: OnceCell<Vec<String>>,
        pub(super) optional_for: OnceCell<Vec<String>>,

        pub(super) files: OnceCell<Vec<String>>,
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

            self.update_version.borrow().as_ref()
                .map_or(flags, |_| flags | PkgFlags::UPDATES)
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
        });

        pkg
    }

    //---------------------------------------
    // Data field properties
    //---------------------------------------
    #[inline]
    fn data(&self) -> &PkgData {
        self.imp().data.get().unwrap()
    }

    pub fn is_installed(&self) -> bool {
        self.data().is_installed
    }

    pub fn description(&self) -> Option<&str> {
        self.data().description.as_deref()
    }

    pub fn popularity(&self) -> Option<&str> {
        self.data().popularity.as_deref()
    }

    pub fn out_of_date(&self) -> Option<i64> {
        self.data().out_of_date
    }

    pub fn out_of_date_string(&self) -> Option<GString> {
        Self::date_to_string(self.data().out_of_date)
    }

    pub fn url(&self) -> Option<&str> {
        self.data().url.as_deref()
    }

    pub fn package_url(&self) -> Option<String> {
        let data = self.data();
        let repo = &data.repository;

        match repo.as_str() {
            "aur" => {
                Some(format!("https://aur.archlinux.org/packages/{}", data.name))
            }
            _ if Pacman::config().read().unwrap().repos.iter().any(|r| &r.name == repo) => {
                Some(format!("https://www.archlinux.org/packages/{}/{}/{}/",
                    repo, data.architecture.clone().unwrap_or_default(), data.name))
            }
            _ => {
                None
            }
        }

    }

    pub fn pkgbuild_url(&self) -> Option<String> {
        let data = self.data();
        let name = data.base.as_ref().unwrap_or(&data.name);
        let repo = &data.repository;

        match repo.as_str() {
            "aur" => {
                let domain = "https://aur.archlinux.org/cgit/aur.git";

                Some(format!("{domain}/tree/PKGBUILD?h={name}"))
            }
            _ if Pacman::config().read().unwrap().repos.iter().any(|r| &r.name == repo) => {
                let domain = "https://gitlab.archlinux.org/archlinux/packaging/packages";

                Some(format!("{domain}/{name}/-/blob/main/PKGBUILD"))
            }
            "local" => {
                None
            }
            _ => {
                Paths::paru().as_ref().ok().and_then(|_| {
                    let repo_dir = Paru::pkgbuild_repo_dir().join(repo);

                    let key = format!("{name}/PKGBUILD");

                    WalkDir::new(repo_dir)
                        .min_depth(1)
                        .into_iter()
                        .flatten()
                        .filter(|entry| entry.file_name() == "PKGBUILD")
                        .find_map(|entry| {
                            entry.path().ends_with(&key).then(|| {
                                format!("file://{}", entry.path().display())
                            })
                        })
                })
            }
        }
    }

    pub fn status(&self) -> &str {
        match self.data().flags {
            PkgFlags::EXPLICIT => "explicit",
            PkgFlags::DEPENDENCY => "dependency",
            PkgFlags::OPTIONAL => "optional",
            PkgFlags::ORPHAN => "orphan",
            _ => ""
        }
    }

    pub fn status_css_classes(&self) -> Vec<&str> {
        match self.data().flags {
            PkgFlags::ORPHAN => vec!["tag", "warning"],
            PkgFlags::EXPLICIT | PkgFlags::DEPENDENCY | PkgFlags::OPTIONAL => vec!["tag", "success"],
            _ => vec![]
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

    pub fn architecture(&self) -> Option<&str> {
        self.data().architecture.as_deref()
    }

    pub fn packager(&self) -> Option<&str> {
        self.data().packager.as_deref()
    }

    pub fn build_date(&self) -> i64 {
        self.data().build_date
    }

    pub fn build_date_string(&self) -> Option<GString> {
        Self::date_to_string(Some(self.data().build_date))
    }

    pub fn install_date(&self) -> Option<i64> {
        self.data().install_date
    }

    pub fn install_date_string(&self) -> Option<GString> {
        Self::date_to_string(self.data().install_date)
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

    pub fn has_script(&self) -> Option<&str> {
        self.data().has_script.as_deref()
    }

    pub fn validation(&self) -> PkgValidation {
        self.data().validation
    }

    //---------------------------------------
    // Alpm handle function
    //---------------------------------------
    fn with_alpm_handle<F, R>(f: F) -> R
    where F: FnOnce(&RefCell<Option<Alpm>>) -> R {
        thread_local! {
            static ALPM_HANDLE: RefCell<Option<Alpm>> = const { RefCell::new(None) };
        }

        ALPM_HANDLE.with(f)
    }

    //---------------------------------------
    // Init alpm handle function
    //---------------------------------------
    pub fn init_alpm_handle() {
        Self::with_alpm_handle(|handle| {
            let alpm_handle = alpm_utils::alpm_with_conf(&Pacman::config().read().unwrap()).ok();

            handle.replace(alpm_handle);
        });
    }

    //---------------------------------------
    // Alpm package helper functions
    //---------------------------------------
    fn alpm_pkg<'a>(&self, handle: &'a Alpm) -> Option<&'a Package> {
        let data = self.data();

        if data.is_installed {
            handle.localdb().pkg(data.name.as_str()).ok()
        } else {
            handle.syncdbs().pkg(data.name.as_str()).ok()
        }
    }

    fn alpm_local_pkg<'a>(&self, handle: &'a Alpm) -> Option<&'a Package> {
        handle.localdb().pkg(self.data().name.as_str()).ok()
    }

    fn alpm_sync_pkg<'a>(&self, handle: &'a Alpm) -> Option<&'a Package> {
        handle.syncdbs().pkg(self.data().name.as_str()).ok()
    }

    //---------------------------------------
    // Properties from alpm handle
    //---------------------------------------
    pub fn required_by(&self) -> &[String] {
        self.imp().required_by.get_or_init(|| {
            Self::with_alpm_handle(|handle| {
                handle.borrow().as_ref()
                    .and_then(|handle| self.alpm_pkg(handle))
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
                    .and_then(|handle| self.alpm_pkg(handle))
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
                    .and_then(|handle| self.alpm_local_pkg(handle))
                    .map(|pkg| {
                        let mut files: Vec<String> = pkg.files().files()
                            .iter()
                            .map(|file| String::from_utf8_lossy(file.name()).into_owned())
                            .collect();

                        files.sort_unstable();

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
                    .and_then(|handle| self.alpm_local_pkg(handle))
                    .map(|pkg| {
                        let mut backup: Vec<PkgBackup> = pkg.backup().iter()
                            .map(|backup| {
                                PkgBackup {
                                    path: backup.name().into(),
                                    hash: backup.hash().into()
                                }
                            })
                            .collect();

                        backup.sort_unstable_by(|backup_a, backup_b| {
                            backup_a.path.partial_cmp(&backup_b.path)
                                .unwrap_or(Ordering::Equal)
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
                .and_then(|handle| self.alpm_sync_pkg(handle))
                .map(|pkg| {
                    PkgHashes {
                        base64_sig: pkg.base64_sig().map(ToOwned::to_owned),
                        sha256sum: pkg.sha256sum().map(ToOwned::to_owned),
                        md5sum: pkg.md5sum().map(ToOwned::to_owned)
                    }
                })
                .unwrap_or_default()
        })
    }

    //---------------------------------------
    // Date to string helper function
    //---------------------------------------
    fn date_to_string(date: Option<i64>) -> Option<GString> {
        date
            .filter(|&date| date != 0)
            .map(|date| {
                glib::DateTime::from_unix_local(date)
                    .and_then(|datetime| datetime.format("%c"))
                    .expect("Failed to format DateTime")
            })
    }

    //---------------------------------------
    // Satisfier functions
    //---------------------------------------
    pub fn has_local_satisfier(search_term: &str) -> bool {
        Self::with_alpm_handle(|handle| {
            handle.borrow().as_ref()
                .and_then(|handle| handle.localdb().pkgs().find_satisfier(search_term))
                .is_some()
        })
    }

    pub fn find_satisfier(search_term: &str, pkg_model: &gio::ListStore) -> Option<Self> {
        Self::with_alpm_handle(|handle| {
            let handle = handle.borrow();
            let handle = handle.as_ref()?;

            let db_pkg = handle.localdb().pkgs().find_satisfier(search_term)
                .or_else(|| handle.syncdbs().find_satisfier(search_term))?;

            pkg_model.find_with(|pkg: &Self| pkg.name() == db_pkg.name())
        })
    }
}
