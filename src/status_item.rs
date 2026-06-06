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
#[derive(Debug, PartialEq)]
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

        pub(super) error_button: RefCell<gtk::MenuButton>,
        pub(super) spinner: RefCell<adw::Spinner>,
        pub(super) count_label: RefCell<gtk::Label>,

        pub(super) error_label: RefCell<gtk::Label>,
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
            .property("suffix", &indicator)
            .build();

        // Store widgets
        let imp = obj.imp();

        let error_button = builder.object::<gtk::MenuButton>("error_button")
            .expect("Failed to get object from builder");

        let spinner = builder.object::<adw::Spinner>("spinner")
            .expect("Failed to get object from builder");

        let count_label = builder.object::<gtk::Label>("count_label")
            .expect("Failed to get object from builder");

        let error_label = builder.object::<gtk::Label>("error_label")
            .expect("Failed to get object from builder");

        imp.error_button.replace(error_button);
        imp.spinner.replace(spinner);
        imp.count_label.replace(count_label);
        imp.error_label.replace(error_label);

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
        let error_button = imp.error_button.borrow();
        let error_label = imp.error_label.borrow();

        imp.spinner.borrow().set_visible(state == StatusItemState::Checking);

        if let StatusItemState::Updates(count, error) = state {
            count_label.set_visible(count != 0);
            count_label.set_label(&count.to_string());

            error_button.set_visible(error.is_some());
            error_label.set_label(&error.unwrap_or_default());
        } else {
            count_label.set_visible(false);

            error_button.set_visible(false);
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
