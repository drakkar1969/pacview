use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::*;

use crate::pkg_object::PkgFlags;

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
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) text_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) error_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) count_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) error_label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        icon: RefCell<String>,
        #[property(get, set)]
        text: RefCell<String>,
        #[property(get, set)]
        count: Cell<u64>,
        #[property(get, set)]
        updating: Cell<bool>,

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
    impl ObjectImpl for FilterRow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().setup_widgets();
        }
    }

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
        glib::Object::builder()
            .property("icon", icon)
            .property("text", text)
            .property("repo-id", repo_id)
            .property("status-id", status_id)
            .build()
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind properties to widgets
        self.bind_property("updating", &imp.stack.get(), "visible_child_name")
            .transform_to(|_, updating: bool| Some(if updating {"spinner"} else {"icon"}))
            .sync_create()
            .build();
        self.bind_property("icon", &imp.image.get(), "icon-name")
            .sync_create()
            .build();
        self.bind_property("text", &imp.text_label.get(), "label")
            .sync_create()
            .build();
        self.bind_property("count", &imp.count_label.get(), "label")
            .transform_to(|_, count: u64| Some(count.to_string()))
            .sync_create()
            .build();
        self.bind_property("count", &imp.count_label.get(), "visible")
            .transform_to(|_, count: u64| Some(count > 0))
            .sync_create()
            .build();
    }

    //---------------------------------------
    // Public set update status function
    //---------------------------------------
    pub fn set_update_status(&self, error_msg: Option<&str>, n_updates: u64) {
        let imp = self.imp();

        self.set_updating(false);
        self.set_count(n_updates);

        if let Some(error_msg) = error_msg {
            self.add_css_class("error");

            imp.error_label.set_label(error_msg);
            imp.error_button.set_visible(true);
        } else {
            self.remove_css_class("error");

            imp.error_label.set_label("");
            imp.error_button.set_visible(false);
        }
    }
}

impl Default for FilterRow {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        Self::new("", "", None, PkgFlags::empty())
    }
}
