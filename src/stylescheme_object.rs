use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// MODULE: StyleSchemeObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::StyleSchemeObject)]
    pub struct StyleSchemeObject {
        #[property(get, set, construct_only)]
        id: RefCell<String>,
        #[property(get, set, construct_only)]
        name: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for StyleSchemeObject {
        const NAME: &'static str = "StyleSchemeObject";
        type Type = super::StyleSchemeObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for StyleSchemeObject {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: StyleSchemeObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct StyleSchemeObject(ObjectSubclass<imp::StyleSchemeObject>);
}

impl StyleSchemeObject {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(id: &str, name: &str) -> Self {
        glib::Object::builder()
            .property("id", id)
            .property("name", name)
            .build()
    }
}
