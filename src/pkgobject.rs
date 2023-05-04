use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use alpm;
use bytesize;

#[glib::flags(name = "PkgStatusFlags")]
pub enum PkgStatusFlags {
    #[flags_value(name = "Installed")]
    EXPLICIT  = 0b00000001,
    #[flags_value(name = "Dependency")]
    DEPENDENCY = 0b00000010,
    #[flags_value(name = "Optional")]
    OPTIONAL   = 0b00000100,
    #[flags_value(name = "Orphan")]
    ORPHAN     = 0b00001000,
    #[flags_value(name = "None")]
    NONE       = 0b00010000,
    #[flags_value(name = "Installed")]
    INSTALLED = Self::EXPLICIT.bits() | Self::DEPENDENCY.bits() | Self::OPTIONAL.bits() | Self::ORPHAN.bits(),
    #[flags_value(name = "All")]
    ALL = Self::INSTALLED.bits() | Self::NONE.bits(),
    #[gflags(name = "Updates")]
    UPDATES    = 0b00100000,
}

impl Default for PkgStatusFlags {
    fn default() -> Self {
        PkgStatusFlags::NONE
    }
}

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::PkgObject)]
    pub struct PkgObject {
        #[property(get, set)]
        pub flags: Cell<PkgStatusFlags>,
        #[property(get, set)]
        pub name: RefCell<Option<String>>,
        #[property(get, set)]
        pub version: RefCell<Option<String>>,
        #[property(get, set)]
        pub repository: RefCell<Option<String>>,
        #[property(get, set)]
        pub status: RefCell<Option<String>>,
        #[property(get, set)]
        pub status_icon: RefCell<Option<String>>,
        #[property(get, set)]
        pub install_date: Cell<i64>,
        #[property(get, set)]
        pub install_date_short: RefCell<Option<String>>,
        #[property(get, set)]
        pub install_size: Cell<i64>,
        #[property(get, set)]
        pub install_size_string: RefCell<Option<String>>,
        #[property(get, set)]
        pub groups: RefCell<Option<String>>,
    }
    
    #[glib::object_subclass]
    impl ObjectSubclass for PkgObject {
        const NAME: &'static str = "PkgObject";
        type Type = super::PkgObject;
    }
    
    impl ObjectImpl for PkgObject {
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
}

glib::wrapper! {
    pub struct PkgObject(ObjectSubclass<imp::PkgObject>);
}

impl PkgObject {
    pub fn new(repository: &str, syncpkg: alpm::Package, localpkg: Option<alpm::Package>) -> Self {
        let mut flags = PkgStatusFlags::NONE;
        let mut install_date = 0;
        let mut install_date_short = String::from("");

        if let Some(pkg) = localpkg {
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

            install_date = pkg.install_date().unwrap_or(0);

            if install_date != 0 {
                let datetime = glib::DateTime::from_unix_local(install_date).expect("error");
    
                let datestring = datetime.format("%Y/%m/%d %H:%M").expect("error");
    
                install_date_short = datestring.to_string()
            }
        }

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

        let mut groups: Vec<&str> = syncpkg.groups().iter().collect();
        groups.sort_unstable();

        glib::Object::builder()
            .property("name", syncpkg.name())
            .property("version", syncpkg.version().as_str())
            .property("repository", repository)
            .property("flags", flags)
            .property("status", status)
            .property("status-icon", status_icon)
            .property("install-date", install_date)
            .property("install-date-short", install_date_short)
            .property("install-size", syncpkg.isize())
            .property("install-size-string", bytesize::to_string(syncpkg.isize() as u64, true))
            .property("groups", groups.join(", "))
            .build()
    }
}
