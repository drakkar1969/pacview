use std::cell::Cell;

use gtk::glib;
use adw::{prelude::SidebarItemExt, subclass::prelude::*};
use gtk::prelude::*;

use crate::{
    pkg_data::PkgFlags,
    status_item_indicator::{StatusItemIndicator, StatusItemState}
};

//------------------------------------------------------------------------------
// MODULE: StatusItem
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::StatusItem)]
    pub struct StatusItem {
        #[property(get, set, construct_only)]
        id: Cell<PkgFlags>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for StatusItem {
        const NAME: &'static str = "StatusItem";
        type Type = super::StatusItem;
        type ParentType = adw::SidebarItem;
    }

    #[glib::derived_properties]
    impl ObjectImpl for StatusItem {}
    impl SidebarItemImpl for StatusItem {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: StatusItem
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct StatusItem(ObjectSubclass<imp::StatusItem>)
        @extends adw::SidebarItem;
}

impl StatusItem {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(icon: &str, title: &str, id: PkgFlags) -> Self {
        let indicator = StatusItemIndicator::default();

        glib::Object::builder()
            .property("icon-name", icon)
            .property("title", title)
            .property("id", id)
            .property("suffix", &indicator)
            .build()
    }

    //---------------------------------------
    // Public activate function
    //---------------------------------------
    pub fn activate(&self) {
        let sidebar = self.section()
            .and_then(|section| section.sidebar())
            .expect("Could not get sidebar");

        if let Some(index) = sidebar.items().iter::<glib::Object>()
            .flatten()
            .position(|obj| {
                let item = obj
                    .downcast::<Self>()
                    .expect("Could not downcast to 'RepoItem'");

                item.id() == self.id()
            }) {
                sidebar.set_selected(index as u32);
                sidebar.emit_by_name::<()>("activated", &[&(index as u32)]);
            }
    }

    //---------------------------------------
    // Public set state function
    //---------------------------------------
    pub fn set_state(&self, state: StatusItemState) {
        let indicator = self.suffix()
            .and_downcast::<StatusItemIndicator>()
            .expect("Could not downcast to 'StatusItemIndicator'");

        indicator.set_state(state);
    }
}

impl Default for StatusItem {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        Self::new("", "", PkgFlags::default())
    }
}
