use std::cell::{Cell, RefCell};

use gtk::glib;
use adw::{prelude::SidebarItemExt, subclass::prelude::*};
use gtk::prelude::*;

use crate::{
    pkg_data::PkgFlags
};

//------------------------------------------------------------------------------
// ENUM: StatusItemState
//------------------------------------------------------------------------------
#[derive(Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum StatusItemState {
    Updates(usize, Option<String>),
    Reset,
    Checking,
}

impl Default for StatusItemState {
    fn default() -> Self {
        Self::Updates(0, None)
    }
}

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

        pub(super) spinner: RefCell<adw::Spinner>,
        pub(super) count_label: RefCell<gtk::Label>,
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
        let builder = gtk::Builder::from_resource("/com/github/PacView/ui/status_item/indicator.ui");

        // Create status item
        let indicator = builder.object::<gtk::Box>("indicator")
            .expect("Failed to get object from builder");

        let obj: Self = glib::Object::builder()
            .property("icon-name", icon)
            .property("title", title)
            .property("id", id)
            .property("drag-motion-activate", false)
            .property("suffix", &indicator)
            .build();

        // Store widgets
        let imp = obj.imp();

        let spinner = builder.object::<adw::Spinner>("spinner")
            .expect("Failed to get object from builder");

        let count_label = builder.object::<gtk::Label>("count_label")
            .expect("Failed to get object from builder");

        imp.spinner.replace(spinner);
        imp.count_label.replace(count_label);

        obj
    }

    //---------------------------------------
    // Public activate function
    //---------------------------------------
    pub fn activate(&self) {
        let sidebar = self.section()
            .and_then(|section| section.sidebar())
            .expect("Failed to get item sidebar");

        sidebar.set_selected(self.index());
        sidebar.emit_by_name::<()>("activated", &[&(self.index())]);
    }

    //---------------------------------------
    // Public set state function
    //---------------------------------------
    pub fn set_state(&self, state: StatusItemState) {
        let imp = self.imp();

        let count_label = imp.count_label.borrow();

        imp.spinner.borrow().set_visible(state == StatusItemState::Checking);

        if let StatusItemState::Updates(count, error) = state {
            let icon = if error.is_some() {
                "status-updates-error-symbolic"
            } else if count == 0 {
                "status-updates-symbolic"
            } else {
                "status-updates-new-symbolic"
            };

            self.set_icon_name(Some(icon));

            count_label.set_visible(count != 0);
            count_label.set_label(&count.to_string());

            self.set_tooltip(error.as_deref());
        } else {
            self.set_icon_name(Some("status-updates-symbolic"));

            count_label.set_visible(false);

            self.set_tooltip(None);
        }
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
