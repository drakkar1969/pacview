use std::cell::RefCell;

use adw::prelude::SidebarItemExt;
use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::*;

//------------------------------------------------------------------------------
// MODULE: RepoItem
//------------------------------------------------------------------------------
mod imp {
    use adw::subclass::prelude::SidebarItemImpl;

    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::RepoItem)]
    pub struct RepoItem {
        #[property(get, set, nullable, construct_only)]
        id: RefCell<Option<String>>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for RepoItem {
        const NAME: &'static str = "RepoItem";
        type Type = super::RepoItem;
        type ParentType = adw::SidebarItem;
    }

    #[glib::derived_properties]
    impl ObjectImpl for RepoItem {}
    impl SidebarItemImpl for RepoItem {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: RepoItem
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct RepoItem(ObjectSubclass<imp::RepoItem>)
        @extends adw::SidebarItem;
}

impl RepoItem {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(icon: &str, title: &str, id: Option<&str>) -> Self {
        glib::Object::builder()
            .property("icon-name", icon)
            .property("title", title)
            .property("id", id)
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
}

impl Default for RepoItem {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        Self::new("", "", None)
    }
}
