use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::*;

use crate::pkg_data::PkgFlags;

//------------------------------------------------------------------------------
// ENUM: Updates
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Updates {
    Output(Option<String>, u32),
    Checking,
}

impl Default for Updates {
    fn default() -> Self {
        Self::Output(None, 0)
    }
}

//------------------------------------------------------------------------------
// MODULE: FilterRow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::FilterRow)]
    #[template(resource = "/com/github/PacView/ui/filter_row.ui")]
    pub struct FilterRow {
        #[template_child]
        pub(super) image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) text_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) error_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) spinner: TemplateChild<adw::Spinner>,
        #[template_child]
        pub(super) count_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) error_label: TemplateChild<gtk::Label>,

        #[property(get, set, nullable)]
        repo_id: RefCell<Option<String>>,
        #[property(get, set)]
        status_id: Cell<PkgFlags>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for FilterRow {
        const NAME: &'static str = "FilterRow";
        type Type = super::FilterRow;
        type ParentType = gtk::ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for FilterRow {}

    impl WidgetImpl for FilterRow {}
    impl ListBoxRowImpl for FilterRow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: FilterRow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct FilterRow(ObjectSubclass<imp::FilterRow>)
        @extends gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl FilterRow {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(icon: &str, text: &str, repo_id: Option<&str>, status_id: PkgFlags) -> Self {
        let obj: Self = glib::Object::builder()
            .property("repo-id", repo_id)
            .property("status-id", status_id)
            .build();

        let imp = obj.imp();

        imp.image.set_icon_name(Some(icon));

        imp.text_label.set_label(text);

        obj
    }

    //---------------------------------------
    // Public set status function
    //---------------------------------------
    pub fn set_status(&self, status: Updates) {
        let imp = self.imp();

        match status {
            Updates::Output(error, count) => {
                imp.spinner.set_visible(false);

                imp.count_label.set_visible(count != 0);
                imp.count_label.set_label(&count.to_string());

                if let Some(error) = error {
                    imp.error_label.set_label(&error);
                    imp.error_button.set_visible(true);
                } else {
                    imp.error_button.set_visible(false);
                }
            },
            Updates::Checking => {
                imp.error_button.set_visible(false);
                imp.count_label.set_visible(false);
                imp.spinner.set_visible(true);
            }
        }
    }
}

impl Default for FilterRow {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        Self::new("", "", None, PkgFlags::default())
    }
}
