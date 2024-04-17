use std::cell::{RefCell, OnceCell};
use std::rc::Rc;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use alpm_utils::DbListExt;

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
        PkgFlags::empty()
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
    pub makedepends: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    pub architecture: String,
    pub packager: String,
    pub build_date: i64,
    pub download_size: i64,
    pub has_script: bool,
    pub sha256sum: String,
    pub md5sum: String,
    pub has_update: bool,
}

impl PkgData {
    //-----------------------------------
    // New functions
    //-----------------------------------
    pub fn from_pkg(syncpkg: &alpm::Package, localpkg: Result<&alpm::Package, alpm::Error>) -> Self {
        // Defaults for package status flags and install date (non-installed)
        let mut flags = PkgFlags::NONE;
        let mut idate = 0;

        // If package is installed, update properties from local package
        if let Ok(pkg) = localpkg {
            // Get status flags
            flags = if pkg.reason() == alpm::PackageReason::Explicit {
                PkgFlags::EXPLICIT
            } else if !pkg.required_by().is_empty() {
                PkgFlags::DEPENDENCY
            } else if !pkg.optional_for().is_empty() {
                PkgFlags::OPTIONAL
            } else {
                PkgFlags::ORPHAN
            };

            // Get installed date
            idate = pkg.install_date().unwrap_or(0);
        }

        // Build PkgData
        Self {
            flags,
            name: syncpkg.name().to_string(),
            version: syncpkg.version().to_string(),
            repository: syncpkg.db().unwrap().name().to_string(),
            status: match flags {
                PkgFlags::EXPLICIT => "explicit",
                PkgFlags::DEPENDENCY => "dependency",
                PkgFlags::OPTIONAL => "optional",
                PkgFlags::ORPHAN => "orphan",
                _ => ""
            }.to_string(),
            status_icon: match flags {
                PkgFlags::EXPLICIT => "pkg-explicit",
                PkgFlags::DEPENDENCY => "pkg-dependency",
                PkgFlags::OPTIONAL => "pkg-optional",
                PkgFlags::ORPHAN => "pkg-orphan",
                _ => ""
            }.to_string(),
            install_date: idate,
            install_size: syncpkg.isize(),
            groups: Self::alpm_list_to_string(&syncpkg.groups()),
            description: syncpkg.desc().unwrap_or_default().to_string(),
            url: syncpkg.url().unwrap_or_default().to_string(),
            licenses: Self::alpm_list_to_string(&syncpkg.licenses()),
            provides: Self::alpm_deplist_to_vec(&syncpkg.provides()),
            depends: Self::alpm_deplist_to_vec(&syncpkg.depends()),
            optdepends: Self::alpm_deplist_to_vec(&syncpkg.optdepends()),
            makedepends: vec![],
            conflicts: Self::alpm_deplist_to_vec(&syncpkg.conflicts()),
            replaces: Self::alpm_deplist_to_vec(&syncpkg.replaces()),
            architecture: syncpkg.arch().unwrap_or_default().to_string(),
            packager: syncpkg.packager().unwrap_or_default().to_string(),
            build_date: syncpkg.build_date(),
            download_size: syncpkg.download_size(),
            has_script: syncpkg.has_scriptlet(),
            sha256sum: syncpkg.sha256sum().unwrap_or_default().to_string(),
            md5sum: syncpkg.md5sum().unwrap_or_default().to_string(),
            has_update: false,
        }
    }

    pub fn from_aur(aurpkg: &raur::ArcPackage) -> Self {
        // Build PkgData
        Self {
            flags: PkgFlags::NONE,
            name: aurpkg.name.to_string(),
            version: aurpkg.version.to_string(),
            repository: "aur".to_string(),
            status: "".to_string(),
            status_icon: "".to_string(),
            install_date: 0,
            install_size: 0,
            groups: Self::aur_vec_to_string(&aurpkg.groups),
            description: aurpkg.description.clone().unwrap_or_default(),
            url: aurpkg.url.clone().unwrap_or_default(),
            licenses: Self::aur_vec_to_string(&aurpkg.license),
            provides: Self::aur_sorted_vec(&aurpkg.provides),
            depends: Self::aur_sorted_vec(&aurpkg.depends),
            optdepends: Self::aur_sorted_vec(&aurpkg.opt_depends),
            makedepends: Self::aur_sorted_vec(&aurpkg.make_depends),
            conflicts: Self::aur_sorted_vec(&aurpkg.conflicts),
            replaces: Self::aur_sorted_vec(&aurpkg.replaces),
            architecture: "".to_string(),
            packager: aurpkg.maintainer.clone().unwrap_or("Unknown Packager".to_string()),
            build_date: 0,
            download_size: 0,
            has_script: false,
            sha256sum: "".to_string(),
            md5sum: "".to_string(),
            has_update: false,
        }
    }

    //-----------------------------------
    // Helper functions
    //-----------------------------------
    fn alpm_list_to_string(list: &alpm::AlpmList<&str>) -> String {
        let mut list_vec: Vec<&str> = list.iter().collect();
        list_vec.sort_unstable();

        list_vec.join(" | ")
    }

    fn alpm_deplist_to_vec(list: &alpm::AlpmList<&alpm::Dep>) -> Vec<String> {
        let mut dep_vec: Vec<String> = list.iter().map(|dep| dep.to_string()).collect();
        dep_vec.sort_unstable();

        dep_vec
    }

    fn aur_vec_to_string(vec: &[String]) -> String {
        let mut list_vec = vec.to_vec();
        list_vec.sort_unstable();

        list_vec.join(" | ")
    }

    fn aur_sorted_vec(vec: &[String]) -> Vec<String> {
        let mut list_vec = vec.to_vec();
        list_vec.sort_unstable();

        list_vec
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
        #[property(name = "repository",   get, set, type = String,     member = repository)]

        // Read-only properties
        #[property(name = "name",           get, type = String,   member = name)]
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
    pub fn new(handle: Option<Rc<alpm::Alpm>>, data: PkgData) -> Self {
        let pkg: Self = glib::Object::builder().build();

        if let Some(handle) = handle {
            pkg.imp().handle.set(handle).unwrap();
        }

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

    pub fn makedepends(&self) -> Vec<String> {
        self.imp().data.borrow().makedepends.to_owned()
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

    //-----------------------------------
    // Public string property getters
    //-----------------------------------
    pub fn install_date_long(&self) -> String {
        Utils::date_to_string(self.install_date(), "%d %B %Y %H:%M")
    }

    pub fn build_date_long(&self) -> String {
        Utils::date_to_string(self.build_date(), "%d %B %Y %H:%M")
    }

    pub fn download_size_string(&self) -> String {
        Utils::size_to_string(self.download_size(), 1)
    }

    //-----------------------------------
    // Pkg from handle helper function
    //-----------------------------------
    fn pkg_from_handle(&self) -> Result<&alpm::Package, alpm::Error> {
        if let Some(handle) = self.imp().handle.get() {
            let pkg = if self.flags().intersects(PkgFlags::INSTALLED) {
                handle.localdb().pkg(self.name())
            } else {
                handle.syncdbs().pkg(self.name())
            };

            pkg
        } else {
            Err(alpm::Error::HandleNull)
        }
    }

    //-----------------------------------
    // Other public property getters
    //-----------------------------------
    pub fn package_url(&self) -> String {
        let default_repos = ["core", "extra", "multilib"];

        if default_repos.contains(&self.repository().as_str()) {
            format!("https://www.archlinux.org/packages/{repo}/{arch}/{name}",
                repo=self.repository(),
                arch=self.architecture(),
                name=self.name())
        } else if &self.repository() == "aur" {
            format!("https://aur.archlinux.org/packages/{name}", name=self.name())
        } else {
            String::from("")
        }
    }

    pub fn required_by(&self) -> Vec<String> {
        let mut required_by: Vec<String> = vec![];

        if let Ok(pkg) = self.pkg_from_handle() {
            required_by.extend(pkg.required_by());
            required_by.sort_unstable();
        }

        required_by
    }

    pub fn optional_for(&self) -> Vec<String> {
        let mut optional_for: Vec<String> = vec![];

        if let Ok(pkg) = self.pkg_from_handle() {
            optional_for.extend(pkg.optional_for());
            optional_for.sort_unstable();
        }

        optional_for
    }

    pub fn files(&self) -> Vec<String> {
        let mut files: Vec<String> = vec![];

        if let Ok(pkg) = self.pkg_from_handle() {
            files.extend(pkg.files().files().iter().map(|file| format!("/{}", file.name())));
            files.sort_unstable();
        }

        files
    }

    pub fn backup(&self) -> Vec<(String, String)> {
        let mut backups: Vec<(String, String)> = vec![];

        if let Ok(pkg) = self.pkg_from_handle() {
            backups.extend(pkg.backup().iter()
                .map(|bck| (format!("/{}", bck.name()), bck.hash().to_string())));
            backups.sort_unstable_by(|(a_file, _), (b_file, _)| a_file.partial_cmp(b_file).unwrap());
        }

        backups
    }
}
