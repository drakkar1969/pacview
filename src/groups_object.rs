use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// MODULE: GroupsObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::GroupsObject)]
    pub struct GroupsObject {
        #[property(get, set)]
        name: RefCell<String>,
        #[property(get, set)]
        status: RefCell<String>,
        #[property(get, set)]
        status_icon: RefCell<String>,
        #[property(get, set)]
        groups: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for GroupsObject {
        const NAME: &'static str = "GroupsObject";
        type Type = super::GroupsObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for GroupsObject {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: GroupsObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct GroupsObject(ObjectSubclass<imp::GroupsObject>);
}

impl GroupsObject {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(name: &str, status: &str, status_icon: &str, groups: &str) -> Self {
        // Build LogObject
        glib::Object::builder()
            .property("name", name)
            .property("status", status)
            .property("status-icon", status_icon)
            .property("groups", groups)
            .build()
    }
}