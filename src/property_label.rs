use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::*;

//------------------------------------------------------------------------------
// MODULE: PropertyLabel
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PropertyLabel)]
    #[template(resource = "/com/github/PacView/ui/property_label.ui")]
    pub struct PropertyLabel {
        #[template_child]
        pub label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        text: RefCell<String>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PropertyLabel {
        const NAME: &'static str = "PropertyLabel";
        type Type = super::PropertyLabel;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PropertyLabel {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().setup_widgets();
        }
    }

    impl WidgetImpl for PropertyLabel {}
    impl BoxImpl for PropertyLabel {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PropertyLabel
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PropertyLabel(ObjectSubclass<imp::PropertyLabel>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PropertyLabel {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(text: &str) -> Self {
        glib::Object::builder()
            .property("text", text)
            .build()
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        // Bind properties to widgets
        self.bind_property("text", &self.imp().label.get(), "label")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }
}
