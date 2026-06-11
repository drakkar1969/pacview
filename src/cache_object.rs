use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// MODULE: CacheObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::CacheObject)]
    pub struct CacheObject {
        #[property(get, set, construct_only)]
        path: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for CacheObject {
        const NAME: &'static str = "CacheObject";
        type Type = super::CacheObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for CacheObject {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: CacheObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct CacheObject(ObjectSubclass<imp::CacheObject>);
}

impl CacheObject {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(path: &str) -> Self {
        // Build CacheObject
        glib::Object::builder()
            .property("path", path)
            .build()
    }
}
