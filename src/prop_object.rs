use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// ENUM: PropType
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PropType")]
pub enum PropType {
    Text = 0,
    Title = 1,
    Link = 2,
    Packager = 3,
    LinkList = 4,
}

impl Default for PropType {
    fn default() -> Self {
        PropType::Text
    }
}

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
        #[property(get, set, nullable)]
        icon: RefCell<Option<String>>,
        #[property(get, set, builder(PropType::default()))]
        ptype: Cell<PropType>,
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
// IMPLEMENTATION: PropObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PropObject(ObjectSubclass<imp::PropObject>);
}

impl PropObject {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(label: &str, value: &str, icon: Option<&str>, ptype: PropType) -> Self {
        // Build PropObject
        glib::Object::builder()
            .property("label", label)
            .property("value", value)
            .property("icon", icon)
            .property("ptype", ptype)
            .build()
    }
}
