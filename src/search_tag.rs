use std::cell::RefCell;

use gtk::glib;
use adw::subclass::prelude::*;
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
        pub(super) label: TemplateChild<gtk::Label>,

        #[property(get, set, nullable)]
        text: RefCell<String>,
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
            klass.set_css_name("searchtag");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SearchTag {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
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
    }
}

impl Default for SearchTag {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
