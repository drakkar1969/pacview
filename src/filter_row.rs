use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use crate::pkgobject::PkgStatusFlags;

mod imp {
    use super::*;

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
        icon: RefCell<Option<String>>,
        #[property(get, set)]
        text: RefCell<Option<String>>,
        #[property(get, set)]
        count: RefCell<Option<String>>,
        #[property(get, set)]
        spinning: Cell<bool>,

        #[property(get, set)]
        repo_id: RefCell<Option<String>>,
        #[property(get, set)]
        status_id: Cell<PkgStatusFlags>,
    }

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
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }
    
        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();
    
            let obj = self.obj();

            obj.setup_self();
        }
    }

    impl WidgetImpl for FilterRow {}
    impl ListBoxRowImpl for FilterRow {}
}

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

    fn setup_self(&self) {
        let imp = self.imp();

        self.bind_property("spinning", &imp.stack.get(), "visible_child_name")
            .transform_to(|_, spinning: bool| {
                Some(if spinning {"spinner"} else {"icon"})
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        self.bind_property("spinning", &imp.spinner.get(), "spinning")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        self.bind_property("icon", &imp.image.get(), "icon-name")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        self.bind_property("text", &imp.text_label.get(), "label")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        self.bind_property("count", &imp.count_label.get(), "label")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        self.bind_property("count", &imp.count_box.get(), "visible")
                .transform_to(|_, count: Option<&str>| {
                    Some(if count != Some("") {true} else {false})
                })
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
    }
}

impl Default for FilterRow {
    fn default() -> Self {
        Self::new("", "")
    }
}