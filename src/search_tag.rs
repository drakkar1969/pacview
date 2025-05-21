use std::cell::RefCell;

use gtk::glib;
use adw::subclass::prelude::*;
use gtk::prelude::*;

//------------------------------------------------------------------------------
// MODULE: SearchTag
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::SearchTag)]
    #[template(resource = "/com/github/PacView/ui/search_tag.ui")]
    pub struct SearchTag {
        #[template_child]
        pub(super) label: TemplateChild<gtk::Label>,

        #[property(get, set = Self::set_text, nullable)]
        text: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
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
    impl ObjectImpl for SearchTag {}

    impl WidgetImpl for SearchTag {}
    impl BinImpl for SearchTag {}

    impl SearchTag {
        fn set_text(&self, text: &str) {
            self.label.set_label(text);
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: SearchTag
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SearchTag(ObjectSubclass<imp::SearchTag>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for SearchTag {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
