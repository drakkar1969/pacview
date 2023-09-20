use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;
use glib::once_cell::sync::OnceCell;

use alpm;

use crate::utils::Utils;

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
        PkgFlags::NONE
    }
}

//------------------------------------------------------------------------------
// STRUCT: PkgBackup
//------------------------------------------------------------------------------
#[derive(Default, Clone)]
pub struct PkgBackup {
    pub filename: String,
    pub hash: String,
}

impl PkgBackup {
    pub fn new(filename: &str, hash: &str) -> Self {
        Self {
            filename: filename.to_string(),
            hash: hash.to_string()
        }
    }
}

//------------------------------------------------------------------------------
// STRUCT: PkgData
//------------------------------------------------------------------------------
#[derive(Default)]
pub struct PkgData {
    pub flags: PkgFlags,
    pub name: String,
    pub version: String,
    pub repository: String,
    pub repo_show: String,
    pub status: String,
    pub status_icon: String,
    pub install_date: i64,
    pub install_size: i64,
    pub groups: String,

    pub description: String,
    pub url: String,
    pub licenses: String,
    pub provides: Vec<String>,
    pub depends: Vec<String>,
    pub optdepends: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    pub architecture: String,
    pub packager: String,
    pub build_date: i64,
    pub download_size: i64,
    pub has_script: bool,
    pub sha256sum: String,
    pub md5sum: String,
    pub files: Vec<String>,
    pub backup: Vec<PkgBackup>,
    pub has_update: bool,
}

impl PkgData {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(syncpkg: alpm::Package, localpkg: Result<alpm::Package, alpm::Error>) -> Self {
        // Defaults for package status flags, install date, files and backup (non-installed)
        let mut flags = PkgFlags::NONE;
        let mut idate = 0;
        let mut files_vec: Vec<String> = vec![];
        let mut backup_vec: Vec<PkgBackup> = vec![];

        // If package is installed, update properties from local package
        if let Ok(pkg) = localpkg {
            // Get status flags
            flags = if pkg.reason() == alpm::PackageReason::Explicit {
                PkgFlags::EXPLICIT
            } else {
                if !pkg.required_by().is_empty() {
                    PkgFlags::DEPENDENCY
                } else {
                    if !pkg.optional_for().is_empty() {PkgFlags::OPTIONAL} else {PkgFlags::ORPHAN}
                }
            };

            // Get installed date
            idate = pkg.install_date().unwrap_or(0);

            // Get files
            files_vec.extend(Self::alpm_filelist_to_vec(&pkg.files()));

            // Get backup
            backup_vec.extend(Self::alpm_backuplist_to_vec(&pkg.backup()));
        }

        // Get package repository
        let repo = syncpkg.db().unwrap().name();

        // Build PkgData
        Self {
            flags,
            name: syncpkg.name().to_string(),
            version: syncpkg.version().to_string(),
            repository: repo.to_string(),
            repo_show: repo.to_string(),
            status: match flags {
                PkgFlags::EXPLICIT => "explicit".to_string(),
                PkgFlags::DEPENDENCY => "dependency".to_string(),
                PkgFlags::OPTIONAL => "optional".to_string(),
                PkgFlags::ORPHAN => "orphan".to_string(),
                _ => "".to_string()
            },
            status_icon: match flags {
                PkgFlags::EXPLICIT => "pkg-explicit".to_string(),
                PkgFlags::DEPENDENCY => "pkg-dependency".to_string(),
                PkgFlags::OPTIONAL => "pkg-optional".to_string(),
                PkgFlags::ORPHAN => "pkg-orphan".to_string(),
                _ => "".to_string()
            },
            install_date: idate,
            install_size: syncpkg.isize(),
            groups: Self::alpm_list_to_string(&syncpkg.groups()),
            description: syncpkg.desc().unwrap_or_default().to_string(),
            url: syncpkg.url().unwrap_or_default().to_string(),
            licenses: Self::alpm_list_to_string(&syncpkg.licenses()),
            provides: Self::alpm_deplist_to_vec(&syncpkg.provides()),
            depends: Self::alpm_deplist_to_vec(&syncpkg.depends()),
            optdepends: Self::alpm_deplist_to_vec(&syncpkg.optdepends()),
            conflicts: Self::alpm_deplist_to_vec(&syncpkg.conflicts()),
            replaces: Self::alpm_deplist_to_vec(&syncpkg.replaces()),
            architecture: syncpkg.arch().unwrap_or_default().to_string(),
            packager: syncpkg.packager().unwrap_or_default().to_string(),
            build_date: syncpkg.build_date(),
            download_size: syncpkg.download_size(),
            has_script: syncpkg.has_scriptlet(),
            sha256sum: syncpkg.sha256sum().unwrap_or_default().to_string(),
            md5sum: syncpkg.md5sum().unwrap_or_default().to_string(),
            files: files_vec,
            backup: backup_vec,
            has_update: false,
        }
    }

    //-----------------------------------
    // Helper functions
    //-----------------------------------
    fn alpm_list_to_string(list: &alpm::AlpmList<&str>) -> String {
        let mut list_vec: Vec<&str> = list.iter().collect();
        list_vec.sort_unstable();

        list_vec.join(", ")
    }

    fn alpm_deplist_to_vec(list: &alpm::AlpmList<alpm::Dep>) -> Vec<String> {
        let mut dep_vec: Vec<String> = list.iter().map(|dep| dep.to_string()).collect();
        dep_vec.sort_unstable();

        dep_vec
    }

    fn alpm_filelist_to_vec(list: &alpm::FileList) -> Vec<String> {
        let mut file_vec: Vec<String> = list.files().iter()
            .map(|file| format!("/{}", file.name()))
            .collect();
        file_vec.sort_unstable();

        file_vec
    }

    fn alpm_backuplist_to_vec(list: &alpm::AlpmList<alpm::Backup>) -> Vec<PkgBackup> {
        let mut backup_vec: Vec<PkgBackup> = list.iter()
            .map(|bck| PkgBackup::new(&format!("/{}", bck.name()), bck.hash()))
            .collect();
        backup_vec.sort_unstable_by(|a, b| a.filename.partial_cmp(&b.filename).unwrap());

        backup_vec
    }
}

//------------------------------------------------------------------------------
// MODULE: PkgObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::PkgObject)]
    pub struct PkgObject {
        // Alpm handle
        pub handle: OnceCell<Rc<alpm::Alpm>>,

        // Read-write properties
        #[property(name = "flags",        get, set, type = PkgFlags,   member = flags)]
        #[property(name = "version",      get, set, type = String,     member = version)]
        #[property(name = "has-update",   get, set, type = bool,       member = has_update)]
        #[property(name = "repo-show",    get, set, type = String,     member = repo_show)]

        // Read-only properties
        #[property(name = "name",           get, type = String,   member = name)]
        #[property(name = "repository",     get, type = String,   member = repository)]
        #[property(name = "status",         get, type = String,   member = status)]
        #[property(name = "status-icon",    get, type = String,   member = status_icon)]
        #[property(name = "install-date",   get, type = i64,      member = install_date)]
        #[property(name = "install-size",   get, type = i64,      member = install_size)]
        #[property(name = "groups",         get, type = String,   member = groups)]
        pub data: RefCell<PkgData>,

        // Read-only properties with custom getter
        #[property(name = "install-date-short", get = Self::install_date_short)]
        _install_date_short: RefCell<String>,
        #[property(name = "install-size-string", get = Self::install_size_string)]
        _install_size_string: RefCell<String>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PkgObject {
        const NAME: &'static str = "PkgObject";
        type Type = super::PkgObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for PkgObject {}

    impl PkgObject {
        //-----------------------------------
        // Read-only property getters
        //-----------------------------------
        fn install_date_short(&self) -> String {
            Utils::date_to_string(self.obj().install_date(), "%Y/%m/%d %H:%M")
        }

        fn install_size_string(&self) -> String {
            Utils::size_to_string(self.obj().install_size(), 1)
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
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(handle: Rc<alpm::Alpm>, data: PkgData) -> Self {
        let pkg: Self = glib::Object::builder().build();

        pkg.imp().handle.set(handle).unwrap();
        pkg.imp().data.replace(data);

        pkg
    }

    //-----------------------------------
    // Public data field getters
    //-----------------------------------
    pub fn description(&self) -> String {
        self.imp().data.borrow().description.to_owned()
    }

    pub fn url(&self) -> String {
        self.imp().data.borrow().url.to_owned()
    }

    pub fn licenses(&self) -> String {
        self.imp().data.borrow().licenses.to_owned()
    }

    pub fn provides(&self) -> Vec<String> {
        self.imp().data.borrow().provides.to_owned()
    }

    pub fn depends(&self) -> Vec<String> {
        self.imp().data.borrow().depends.to_owned()
    }

    pub fn optdepends(&self) -> Vec<String> {
        self.imp().data.borrow().optdepends.to_owned()
    }

    pub fn conflicts(&self) -> Vec<String> {
        self.imp().data.borrow().conflicts.to_owned()
    }

    pub fn replaces(&self) -> Vec<String> {
        self.imp().data.borrow().replaces.to_owned()
    }

    pub fn architecture(&self) -> String {
        self.imp().data.borrow().architecture.to_owned()
    }

    pub fn packager(&self) -> String {
        self.imp().data.borrow().packager.to_owned()
    }

    pub fn build_date(&self) -> i64 {
        self.imp().data.borrow().build_date
    }

    pub fn download_size(&self) -> i64 {
        self.imp().data.borrow().download_size
    }

    pub fn has_script(&self) -> bool {
        self.imp().data.borrow().has_script
    }

    pub fn sha256sum(&self) -> String {
        self.imp().data.borrow().sha256sum.to_owned()
    }

    pub fn md5sum(&self) -> String {
        self.imp().data.borrow().md5sum.to_owned()
    }

    pub fn files(&self) -> Vec<String> {
        self.imp().data.borrow().files.to_owned()
    }

    pub fn backup(&self) -> Vec<PkgBackup> {
        self.imp().data.borrow().backup.to_owned()
    }

    pub fn install_date_long(&self) -> String {
        Utils::date_to_string(self.install_date(), "%d %B %Y %H:%M")
    }

    pub fn build_date_long(&self) -> String {
        Utils::date_to_string(self.build_date(), "%d %B %Y %H:%M")
    }

    pub fn download_size_string(&self) -> String {
        Utils::size_to_string(self.download_size(), 1)
    }

    pub fn required_by(&self) -> Vec<String> {
        let mut required_by: Vec<String> = vec![];

        if let Some(handle) = self.imp().handle.get() {
            let db = if self.flags().intersects(PkgFlags::INSTALLED) {
                Some(handle.localdb())
            } else {
                handle.syncdbs().iter().find(|db| db.name() == self.repository())
            };

            if let Some(db) = db {
                if let Ok(pkg) = db.pkg(self.name()) {
                    required_by.extend(pkg.required_by());
                    required_by.sort_unstable();
                }
            }
        }

        required_by
    }

    pub fn optional_for(&self) -> Vec<String> {
        let mut optional_for: Vec<String> = vec![];

        if let Some(handle) = self.imp().handle.get() {
            let db = if self.flags().intersects(PkgFlags::INSTALLED) {
                Some(handle.localdb())
            } else {
                handle.syncdbs().iter().find(|db| db.name() == self.repository())
            };

            if let Some(db) = db {
                if let Ok(pkg) = db.pkg(self.name()) {
                    optional_for.extend(pkg.optional_for());
                    optional_for.sort_unstable();
                }
            }
        }

        optional_for
    }
}
