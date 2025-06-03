use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// MODULE: LogObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::LogObject)]
    pub struct LogObject {
        #[property(get, set, construct_only)]
        date: RefCell<String>,
        #[property(get, set, construct_only)]
        time: RefCell<String>,
        #[property(get, set, construct_only)]
        category: RefCell<String>,
        #[property(get, set, construct_only)]
        message: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for LogObject {
        const NAME: &'static str = "LogObject";
        type Type = super::LogObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for LogObject {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: LogObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct LogObject(ObjectSubclass<imp::LogObject>);
}

impl LogObject {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(date: &str, time: &str, category: &str, message: &str) -> Self {
        // Build LogObject
        glib::Object::builder()
            .property("date", date)
            .property("time", time)
            .property("category", category)
            .property("message", message)
            .build()
    }
}
