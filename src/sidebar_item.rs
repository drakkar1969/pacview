use std::cell::Cell;

use gtk::glib;
use adw::{prelude::SidebarItemExt, subclass::prelude::*};
use gtk::prelude::*;

use crate::{
    pkg_data::PkgFlags,
    sidebar_item_indicator::{SidebarItemIndicator, SidebarItemState}
};

//------------------------------------------------------------------------------
// MODULE: SidebarItem
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::SidebarItem)]
    pub struct SidebarItem {
        #[property(get, set, construct_only)]
        id: Cell<PkgFlags>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for SidebarItem {
        const NAME: &'static str = "SidebarItem";
        type Type = super::SidebarItem;
        type ParentType = adw::SidebarItem;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SidebarItem {}
    impl SidebarItemImpl for SidebarItem {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: SidebarItem
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SidebarItem(ObjectSubclass<imp::SidebarItem>)
        @extends adw::SidebarItem;
}

impl SidebarItem {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(icon: &str, title: &str, id: PkgFlags) -> Self {
        let indicator = SidebarItemIndicator::default();

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
    pub fn set_state(&self, state: SidebarItemState) {
        let indicator = self.suffix()
            .and_downcast::<SidebarItemIndicator>()
            .expect("Could not downcast to 'SidebarItemIndicator'");

        indicator.set_state(state);
    }
}

impl Default for SidebarItem {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        Self::new("", "", PkgFlags::default())
    }
}
