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
        repository: RefCell<String>,
        #[property(get, set)]
        packages: RefCell<String>,
        #[property(get, set)]
        installed: RefCell<String>,
        #[property(get, set)]
        size: RefCell<String>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for StatsObject {
        const NAME: &'static str = "StatsObject";
        type Type = super::StatsObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for StatsObject {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: StatsObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct StatsObject(ObjectSubclass<imp::StatsObject>);
}

impl StatsObject {
    //-----------------------------------
    // New function
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
