use std::cell::RefCell;
use std::marker::PhantomData;

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
        filename: RefCell<String>,
        #[property(get = Self::get_icon)]
        icon: PhantomData<String>,
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

    impl CacheObject {
        //---------------------------------------
        // Property getter
        //---------------------------------------
        fn get_icon(&self) -> String {
            if self.filename.borrow().ends_with(".sig") {
                "info-signed-symbolic"
            } else {
                "info-archive-symbolic"
            }.to_owned()
        }
    }
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
    pub fn new(filename: &str) -> Self {
        // Build CacheObject
        glib::Object::builder()
            .property("filename", filename)
            .build()
    }
}
