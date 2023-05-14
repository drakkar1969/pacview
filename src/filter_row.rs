use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::*;

use crate::pkg_object::PkgFlags;

//------------------------------------------------------------------------------
// MODULE: FILTERROW
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::FilterRow)]
    #[template(resource = "/com/github/PacView/ui/filter_row.ui")]
    pub struct FilterRow {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub image: TemplateChild<gtk::Image>,
        #[template_child]
        pub spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub text_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub count_box: TemplateChild<gtk::Box>,

        #[property(get, set)]
        icon: RefCell<String>,
        #[property(get, set)]
        text: RefCell<String>,
        #[property(get, set)]
        count: RefCell<String>,
        #[property(get, set)]
        spinning: Cell<bool>,

        pub repo_id: RefCell<String>,
        pub status_id: Cell<PkgFlags>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
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

    impl ObjectImpl for FilterRow {
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

        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // Bind properties to widgets
            obj.bind_property("spinning", &self.stack.get(), "visible_child_name")
                .transform_to(|_, spinning: bool| {
                    Some(if spinning {"spinner"} else {"icon"})
                })
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
            obj.bind_property("spinning", &self.spinner.get(), "spinning")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
            obj.bind_property("icon", &self.image.get(), "icon-name")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
            obj.bind_property("text", &self.text_label.get(), "label")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
            obj.bind_property("count", &self.count_label.get(), "label")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
            obj.bind_property("count", &self.count_box.get(), "visible")
                .transform_to(|_, count: &str| {
                    Some(if count != "" {true} else {false})
                })
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        }
    }

    impl WidgetImpl for FilterRow {}
    impl ListBoxRowImpl for FilterRow {}
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct FilterRow(ObjectSubclass<imp::FilterRow>)
        @extends gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl FilterRow {
    pub fn new(icon: &str, text: &str) -> Self {
        glib::Object::builder()
            .property("icon", icon)
            .property("text", text)
            .build()
    }

    //-----------------------------------
    // Public id getters/setters
    //-----------------------------------
    pub fn repo_id(&self) -> String {
        self.imp().repo_id.borrow().to_string()
    }

    pub fn set_repo_id(&self, id: &str) {
        self.imp().repo_id.replace(id.to_string());
    }

    pub fn status_id(&self) -> PkgFlags {
        self.imp().status_id.get()
    }

    pub fn set_status_id(&self, id: PkgFlags) {
        self.imp().status_id.replace(id);
    }
}

impl Default for FilterRow {
    fn default() -> Self {
        Self::new("", "")
    }
}
