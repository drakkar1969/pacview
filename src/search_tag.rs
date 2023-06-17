use std::cell::{Cell, RefCell};

use gtk::glib;
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

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
        #[template_child]
        pub button: TemplateChild<gtk::Button>,

        #[property(get, set)]
        text: RefCell<Option<String>>,
        #[property(get, set, default = true, construct)]
        can_close: Cell<bool>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for SearchTag {
        const NAME: &'static str = "SearchTag";
        type Type = super::SearchTag;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
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

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for SearchTag {}
    impl BinImpl for SearchTag {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: SearchTag
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SearchTag(ObjectSubclass<imp::SearchTag>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SearchTag {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind properties to widgets
        self.bind_property("text", &imp.label.get(), "label")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        self.bind_property("can-close", &imp.button.get(), "visible")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        // Close button clicked signal
        self.imp().button.connect_clicked(clone!(@weak self as obj => move |_| {
            obj.set_visible(false);
        }));
    }
}
