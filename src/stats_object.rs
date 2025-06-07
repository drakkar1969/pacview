use std::cell::RefCell;
use std::marker::PhantomData;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// MODULE: StatsObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::StatsObject)]
    pub struct StatsObject {
        #[property(get, set, nullable, construct_only)]
        icon: RefCell<Option<String>>,
        #[property(get = Self::icon_visible)]
        icon_visible: PhantomData<bool>,
        #[property(get, set, construct_only)]
        repository: RefCell<String>,
        #[property(get, set, construct_only)]
        packages: RefCell<String>,
        #[property(get, set, construct_only)]
        installed: RefCell<String>,
        #[property(get, set, construct_only)]
        size: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for StatsObject {
        const NAME: &'static str = "StatsObject";
        type Type = super::StatsObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for StatsObject {}

    impl StatsObject {
        //---------------------------------------
        // Property getter
        //---------------------------------------
        fn icon_visible(&self) -> bool {
            self.icon.borrow().is_some()
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: StatsObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct StatsObject(ObjectSubclass<imp::StatsObject>);
}

impl StatsObject {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(icon: Option<&str>, repository: &str, packages: &str, installed: &str, size: &str) -> Self {
        // Build StatsObject
        glib::Object::builder()
            .property("icon", icon)
            .property("repository", repository)
            .property("packages", packages)
            .property("installed", installed)
            .property("size", size)
            .build()
    }
}
