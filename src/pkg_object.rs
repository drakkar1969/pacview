use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use alpm;
use bytesize;

//------------------------------------------------------------------------------
// FLAGS: PKGSTATUSFLAGS
//------------------------------------------------------------------------------
#[glib::flags(name = "PkgStatusFlags")]
pub enum PkgStatusFlags {
    #[flags_value(name = "All")]
    ALL = Self::INSTALLED.bits() | Self::NONE.bits(),
    #[flags_value(name = "Installed")]
    INSTALLED = Self::EXPLICIT.bits() | Self::DEPENDENCY.bits() | Self::OPTIONAL.bits() | Self::ORPHAN.bits(),
    #[flags_value(name = "Explicit")]
    EXPLICIT  = 0b00000001,
    #[flags_value(name = "Dependency")]
    DEPENDENCY = 0b00000010,
    #[flags_value(name = "Optional")]
    OPTIONAL   = 0b00000100,
    #[flags_value(name = "Orphan")]
    ORPHAN     = 0b00001000,
    #[flags_value(name = "None")]
    NONE       = 0b00010000,
    #[gflags(name = "Updates")]
    UPDATES    = 0b00100000,
}

impl Default for PkgStatusFlags {
    fn default() -> Self {
        PkgStatusFlags::NONE
    }
}

//------------------------------------------------------------------------------
// MODULE: PKGOBJECT
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::PkgObject)]
    pub struct PkgObject {
        #[property(get, set)]
        pub flags: Cell<PkgStatusFlags>,
        #[property(get, set)]
        pub name: RefCell<String>,
        #[property(get, set)]
        pub version: RefCell<String>,
        #[property(get, set)]
        pub repository: RefCell<String>,
        #[property(get, set)]
        pub status: RefCell<String>,
        #[property(get, set)]
        pub status_icon: RefCell<String>,
        #[property(get, set)]
        pub install_date: Cell<i64>,
        #[property(get = Self::install_date_short)] // Read-only, custom getter
        pub install_date_short: RefCell<String>,
        #[property(get = Self::install_date_long)] // Read-only, custom getter
        pub install_date_long: RefCell<String>,
        #[property(get, set)]
        pub install_size: Cell<i64>,
        #[property(get = Self::install_size_string)] // Read-only, custom getter
        pub install_size_string: RefCell<String>,
        #[property(get, set)]
        pub groups: RefCell<String>,

        #[property(get, set)]
        pub description: RefCell<String>,
        #[property(get, set)]
        pub url: RefCell<String>,
        #[property(get, set)]
        pub licenses: RefCell<String>,
        #[property(get, set)]
        pub depends: RefCell<Vec<String>>,
        #[property(get, set)]
        pub optdepends: RefCell<Vec<String>>,
        #[property(get, set)]
        pub provides: RefCell<Vec<String>>,
        #[property(get, set)]
        pub conflicts: RefCell<Vec<String>>,
        #[property(get, set)]
        pub replaces: RefCell<Vec<String>>,
        #[property(get, set)]
        pub architecture: RefCell<String>,
        #[property(get, set)]
        pub packager: RefCell<String>,
        #[property(get, set)]
        pub build_date: Cell<i64>,
        #[property(get = Self::build_date_long)] // Read-only, custom getter
        pub build_date_long: RefCell<String>,
        #[property(get, set)]
        pub download_size: Cell<i64>,
        #[property(get = Self::download_size_string)] // Read-only, custom getter
        pub download_size_string: RefCell<String>,
        #[property(get, set)]
        pub has_script: Cell<bool>,
        #[property(get, set)]
        pub sha256sum: RefCell<String>,
        #[property(get, set)]
        pub md5sum: RefCell<String>,
        #[property(get, set)]
        pub files: RefCell<Vec<String>>,
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
            let obj = self.obj();

            if obj.install_date() == 0 {
                String::from("")
            }
            else {
                self.date_to_string(self.obj().install_date(), "%Y/%m/%d %H:%M")
            }
        }

        fn install_date_long(&self) -> String {
            let obj = self.obj();

            if obj.install_date() == 0 {
                String::from("")
            }
            else {
                self.date_to_string(self.obj().install_date(), "%d %B %Y %H:%M")
            }
        }

        fn install_size_string(&self) -> String {
            bytesize::to_string(self.obj().install_size() as u64, true)
        }

        fn build_date_long(&self) -> String {
            self.date_to_string(self.obj().build_date(), "%d %B %Y %H:%M")
        }

        fn download_size_string(&self) -> String {
            bytesize::to_string(self.obj().download_size() as u64, true)
        }

        //-----------------------------------
        // Helper functions
        //-----------------------------------
        fn date_to_string(&self, date: i64, format: &str) -> String {
            let datetime = glib::DateTime::from_unix_local(date).expect("error");

            datetime.format(format).expect("error").to_string()
        }
    }
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PkgObject(ObjectSubclass<imp::PkgObject>);
}

impl PkgObject {
    pub fn new(repository: &str, syncpkg: alpm::Package, localpkg: Result<alpm::Package, alpm::Error>) -> Self {
        // Default values for package status flags, install date and files (non-installed)
        let mut flags = PkgStatusFlags::NONE;
        let mut install_date = 0;
        let mut file_vec: Vec<String> = Vec::new();

        // If package is installed locally
        if let Ok(pkg) = localpkg {
            // Get package status flags
            if pkg.reason() == alpm::PackageReason::Explicit {
                flags = PkgStatusFlags::EXPLICIT;
            } else {
                if !pkg.required_by().is_empty() {
                    flags = PkgStatusFlags::DEPENDENCY;
                } else {
                    if !pkg.optional_for().is_empty() {
                        flags = PkgStatusFlags::OPTIONAL;
                    } else {
                        flags = PkgStatusFlags::ORPHAN;
                    }
                }
            }

            // Get package installed date
            install_date = pkg.install_date().unwrap_or(0);

            // Get package files
            file_vec.extend(PkgObject::filelist_to_vec(&pkg.files()));
        }

        // Get package status and status icon
        let status = match flags {
            PkgStatusFlags::EXPLICIT => "explicit",
            PkgStatusFlags::DEPENDENCY => "dependency",
            PkgStatusFlags::OPTIONAL => "optional",
            PkgStatusFlags::ORPHAN => "orphan",
            _ => ""
        };

        let status_icon = match flags {
            PkgStatusFlags::EXPLICIT => "pkg-explicit",
            PkgStatusFlags::DEPENDENCY => "pkg-dependency",
            PkgStatusFlags::OPTIONAL => "pkg-optional",
            PkgStatusFlags::ORPHAN => "pkg-orphan",
            _ => ""
        };

        // Get package groups
        let mut group_list: Vec<&str> = syncpkg.groups().iter().collect();
        group_list.sort_unstable();

        let groups = group_list.join(", ");

        // Get package licenses
        let mut license_list: Vec<&str> = syncpkg.licenses().iter().collect();
        license_list.sort_unstable();

        let licenses = license_list.join(", ");

        // Build PkgObject
        glib::Object::builder()
            .property("name", syncpkg.name())
            .property("version", syncpkg.version().as_str())
            .property("repository", repository)
            .property("flags", flags)
            .property("status", status)
            .property("status-icon", status_icon)
            .property("install-date", install_date)
            .property("install-size", syncpkg.isize())
            .property("groups", groups)

            .property("description", syncpkg.desc().unwrap_or_default())
            .property("url", syncpkg.url().unwrap_or_default())
            .property("licenses", licenses)
            .property("depends", PkgObject::deplist_to_vec(&syncpkg.depends()))
            .property("optdepends", PkgObject::deplist_to_vec(&syncpkg.optdepends()))
            .property("provides", PkgObject::deplist_to_vec(&syncpkg.provides()))
            .property("conflicts", PkgObject::deplist_to_vec(&syncpkg.conflicts()))
            .property("replaces", PkgObject::deplist_to_vec(&syncpkg.replaces()))
            .property("architecture", syncpkg.arch().unwrap_or_default())
            .property("packager", syncpkg.packager().unwrap_or_default())
            .property("build-date", syncpkg.build_date())
            .property("download-size", syncpkg.download_size())
            .property("has-script", syncpkg.has_scriptlet())
            .property("sha256sum", syncpkg.sha256sum().unwrap_or_default())
            .property("md5sum", syncpkg.md5sum().unwrap_or_default())
            .property("files", file_vec)

            .build()
    }

    fn deplist_to_vec(list: &alpm::AlpmList<alpm::Dep>) -> Vec<String> {
        let mut dep_vec: Vec<String> = list.iter().map(|dep| dep.to_string()).collect();
        dep_vec.sort_unstable();

        dep_vec
    }

    fn filelist_to_vec(list: &alpm::FileList) -> Vec<String> {
        let mut file_vec: Vec<String> = list.files().iter().map(|file| file.name().to_string()).collect();
        file_vec.sort_unstable();

        file_vec
    }
}