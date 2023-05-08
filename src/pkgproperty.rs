use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// MODULE: PKGPROPERTY
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::PkgProperty)]
    pub struct PkgProperty {
        #[property(get, set)]
        pub label: RefCell<Option<String>>,
        #[property(get, set)]
        pub value: RefCell<Option<String>>,
        #[property(get, set)]
        pub icon: RefCell<Option<String>>,
    }
    
    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PkgProperty {
        const NAME: &'static str = "PkgProperty";
        type Type = super::PkgProperty;
    }
    
    impl ObjectImpl for PkgProperty {
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
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PkgProperty(ObjectSubclass<imp::PkgProperty>);
}

impl PkgProperty {
    pub fn new(label: &str, value: &str, icon: &str) -> Self {
        // Build PkgProperty
        glib::Object::builder()
            .property("label", label)
            .property("value", value)
            .property("icon", icon)
            .build()
    }
}
