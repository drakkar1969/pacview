use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// MODULE: StatsObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::StatsObject)]
    pub struct StatsObject {
        #[property(get, set)]
        pub repository: RefCell<String>,
        #[property(get, set)]
        pub packages: RefCell<String>,
        #[property(get, set)]
        pub installed: RefCell<String>,
        #[property(get, set)]
        pub size: RefCell<String>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for StatsObject {
        const NAME: &'static str = "StatsObject";
        type Type = super::StatsObject;
    }

    impl ObjectImpl for StatsObject {
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
// PUBLIC IMPLEMENTATION: StatsObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct StatsObject(ObjectSubclass<imp::StatsObject>);
}

impl StatsObject {
    //-----------------------------------
    // Public new function
    //-----------------------------------
    pub fn new(repository: &str, packages: &str, installed: &str, size: &str) -> Self {
        // Build StatsObject
        glib::Object::builder()
            .property("repository", repository)
            .property("packages", packages)
            .property("installed", installed)
            .property("size", size)
            .build()
    }
}
