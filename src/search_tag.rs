use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::*;

//------------------------------------------------------------------------------
// MODULE: SearchTag
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::SearchTag)]
    #[template(resource = "/com/github/PacView/ui/search_tag.ui")]
    pub struct SearchTag {
        #[template_child]
        pub label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        text: RefCell<Option<String>>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for SearchTag {
        const NAME: &'static str = "SearchTag";
        type Type = super::SearchTag;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SearchTag {
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

            // Bind properties to widgets
            self.obj().bind_property("text", &self.label.get(), "label")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        }
    }

    impl WidgetImpl for SearchTag {}
    impl BoxImpl for SearchTag {}

    #[gtk::template_callbacks]
    impl SearchTag {
        //-----------------------------------
        // Signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_close_button_clicked(&self) {
            self.obj().set_visible(false);
        }
    }
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION: SearchTag
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SearchTag(ObjectSubclass<imp::SearchTag>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl SearchTag {
    pub fn new() -> Self {
        glib::Object::builder()
            .build()
    }
}
