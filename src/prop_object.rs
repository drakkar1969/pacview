use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// MODULE: PropObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::PropObject)]
    pub struct PropObject {
        #[property(get, set)]
        label: RefCell<String>,
        #[property(get, set)]
        value: RefCell<String>,
        #[property(get, set)]
        icon: RefCell<Option<String>>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PropObject {
        const NAME: &'static str = "PropObject";
        type Type = super::PropObject;
    }

    impl ObjectImpl for PropObject {
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
// PUBLIC IMPLEMENTATION: PropObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PropObject(ObjectSubclass<imp::PropObject>);
}

impl PropObject {
    //-----------------------------------
    // Public new function
    //-----------------------------------
    pub fn new(label: &str, value: &str, icon: Option<&str>) -> Self {
        // Build PropObject
        glib::Object::builder()
            .property("label", label)
            .property("value", value)
            .property("icon", icon)
            .build()
    }
}
