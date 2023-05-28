use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::*;

//------------------------------------------------------------------------------
// MODULE: ToggleButton
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::ToggleButton)]
    #[template(resource = "/com/github/PacView/ui/toggle_button.ui")]
    pub struct ToggleButton {
        #[template_child]
        pub image: TemplateChild<gtk::Image>,
        #[template_child]
        pub label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        icon: RefCell<String>,
        #[property(get, set)]
        text: RefCell<String>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for ToggleButton {
        const NAME: &'static str = "ToggleButton";
        type Type = super::ToggleButton;
        type ParentType = gtk::ToggleButton;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ToggleButton {
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
            obj.bind_property("icon", &self.image.get(), "icon-name")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
            obj.bind_property("text", &self.label.get(), "label")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        }
    }

    impl WidgetImpl for ToggleButton {}
    impl ButtonImpl for ToggleButton {}
    impl ToggleButtonImpl for ToggleButton {}
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION: ToggleButton
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct ToggleButton(ObjectSubclass<imp::ToggleButton>)
        @extends gtk::ToggleButton, gtk::Button, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl ToggleButton {
    pub fn new(icon: &str, text: &str) -> Self {
        glib::Object::builder()
            .property("icon", icon)
            .property("text", text)
            .build()
    }
}
