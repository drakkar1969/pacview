use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

mod imp {
    use super::*;

    #[derive(glib::Properties, Default)]
    #[properties(wrapper_type = super::PkgObject)]
    pub struct PkgObject {
        #[property(get, set)]
        name: RefCell<Option<String>>,
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
