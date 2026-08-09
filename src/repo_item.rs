use std::cell::RefCell;

use adw::prelude::SidebarItemExt;
use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::*;

//------------------------------------------------------------------------------
// ENUM: RepoItemState
//------------------------------------------------------------------------------
#[derive(Default, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum RepoItemState {
    #[default]
    Reset,
    Searching,
    AurResults(u32),
}

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

        pub(super) spinner: RefCell<adw::Spinner>,
        pub(super) count_label: RefCell<gtk::Label>,
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
        let builder = gtk::Builder::from_resource("/com/github/PacView/ui/sidebar/indicator.ui");

        // Create indicator
        let indicator = builder.object::<gtk::Box>("indicator")
            .expect("Failed to get object from builder");

        // Create status item
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
    pub fn set_state(&self, state: RepoItemState) {
        let imp = self.imp();

        let count_label = imp.count_label.borrow();

        imp.spinner.borrow().set_visible(state == RepoItemState::Searching);

        if let RepoItemState::AurResults(count) = state {
            count_label.set_visible(count != 0);
            count_label.set_label(&format!("+{count}"));
        } else {
            count_label.set_visible(false);
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
