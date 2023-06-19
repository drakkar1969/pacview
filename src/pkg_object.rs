use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

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
        // Read-write properties
        #[property(name = "flags",      get, set, type = PkgFlags, member = flags)]
        #[property(name = "version",    get, set, type = String,   member = version)]
        #[property(name = "has-update", get, set, type = bool,     member = has_update)]
        #[property(name = "repo-show",  get, set, type = String,   member = repo_show)]

        // Read-only properties
        #[property(name = "name",          get, type = String,      member = name)]
        #[property(name = "repository",    get, type = String,      member = repository)]
        #[property(name = "status",        get, type = String,      member = status)]
        #[property(name = "status-icon",   get, type = String,      member = status_icon)]
        #[property(name = "install-date",  get, type = i64,         member = install_date)]
        #[property(name = "install-size",  get, type = i64,         member = install_size)]
        #[property(name = "groups",        get, type = String,      member = groups)]
        #[property(name = "description",   get, type = String,      member = description)]
        #[property(name = "url",           get, type = String,      member = url)]
        #[property(name = "licenses",      get, type = String,      member = licenses)]
        #[property(name = "provides",      get, type = Vec<String>, member = provides)]
        #[property(name = "depends",       get, type = Vec<String>, member = depends)]
        #[property(name = "optdepends",    get, type = Vec<String>, member = optdepends)]
        #[property(name = "conflicts",     get, type = Vec<String>, member = conflicts)]
        #[property(name = "replaces",      get, type = Vec<String>, member = replaces)]
        #[property(name = "architecture",  get, type = String,      member = architecture)]
        #[property(name = "packager",      get, type = String,      member = packager)]
        #[property(name = "build-date",    get, type = i64,         member = build_date)]
        #[property(name = "download-size", get, type = i64,         member = download_size)]
        #[property(name = "has-script",    get, type = bool,        member = has_script)]
        #[property(name = "sha256sum",     get, type = String,      member = sha256sum)]
        #[property(name = "md5sum",        get, type = String,      member = md5sum)]
        pub data: RefCell<PkgData>,

        // Read-only properties with custom getter
        #[property(get = Self::install_date_short)]
        _install_date_short: RefCell<String>,
        #[property(get = Self::install_date_long)]
        _install_date_long: RefCell<String>,
        #[property(get = Self::install_size_string)]
        _install_size_string: RefCell<String>,
        #[property(get = Self::build_date_long)]
        _build_date_long: RefCell<String>,
        #[property(get = Self::download_size_string)]
        _download_size_string: RefCell<String>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PkgObject {
        const NAME: &'static str = "PkgObject";
        type Type = super::PkgObject;
    }

    impl ObjectImpl for PkgObject {
        //-----------------------------------
        // Default property functions
        //-----------------------------------
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }
    }

    impl PkgObject {
        //-----------------------------------
        // Read-only property getters
        //-----------------------------------
        fn install_date_short(&self) -> String {
            Utils::date_to_string(self.obj().install_date(), "%Y/%m/%d %H:%M")
        }

        fn install_date_long(&self) -> String {
            Utils::date_to_string(self.obj().install_date(), "%d %B %Y %H:%M")
        }

        fn install_size_string(&self) -> String {
            Utils::size_to_string(self.obj().install_size(), 1)
        }

        fn build_date_long(&self) -> String {
            Utils::date_to_string(self.obj().build_date(), "%d %B %Y %H:%M")
        }

        fn download_size_string(&self) -> String {
            Utils::size_to_string(self.obj().download_size(), 1)
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
    pub fn new(data: PkgData) -> Self {
        let pkg: Self = glib::Object::builder().build();
        pkg.imp().data.replace(data);
        pkg
    }

    //-----------------------------------
    // Public files function
    //-----------------------------------
    pub fn files(&self) -> Vec<String> {
        let data = self.imp().data.borrow();

        data.files.to_owned()
    }

    //-----------------------------------
    // Public backup function
    //-----------------------------------
    pub fn backup(&self) -> Vec<PkgBackup> {
        let data = self.imp().data.borrow();

        data.backup.to_owned()
    }

    //-----------------------------------
    // Public compute requirements function
    //-----------------------------------
    pub fn compute_requirements(&self, alpm_handle: &Option<alpm::Alpm>) -> (Vec<String>, Vec<String>) {
        let mut required_by: Vec<String> = vec![];
        let mut optional_for: Vec<String> = vec![];

        if let Some(handle) = alpm_handle {
            let db = if self.flags().intersects(PkgFlags::INSTALLED) {
                Some(handle.localdb())
            } else {
                handle.syncdbs().iter().find(|db| db.name() == self.repository())
            };

            if let Some(db) = db {
                if let Ok(pkg) = db.pkg(self.name()) {
                    required_by.extend(pkg.required_by());
                    required_by.sort_unstable();

                    optional_for.extend(pkg.optional_for());
                    optional_for.sort_unstable();
                }
            }
        }

        (required_by, optional_for)
    }
}

impl Default for PkgObject {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new(PkgData::default())
    }
}
