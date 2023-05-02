use std::cell::{Cell,RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

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

    #[derive(glib::Properties, Default)]
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
    }
    
    // The central trait for subclassing a GObject
    #[glib::object_subclass]
    impl ObjectSubclass for PkgObject {
        const NAME: &'static str = "PkgObject";
        type Type = super::PkgObject;
    }
    
    // Trait shared by all GObjects
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
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
