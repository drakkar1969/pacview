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
        pub content: TemplateChild<adw::ButtonContent>,

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

    #[glib::derived_properties]
    impl ObjectImpl for ToggleButton {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().setup_widgets();
        }
    }

    impl WidgetImpl for ToggleButton {}
    impl ButtonImpl for ToggleButton {}
    impl ToggleButtonImpl for ToggleButton {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: ToggleButton
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct ToggleButton(ObjectSubclass<imp::ToggleButton>)
        @extends gtk::ToggleButton, gtk::Button, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl ToggleButton {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(icon: &str, text: &str) -> Self {
        glib::Object::builder()
            .property("icon", icon)
            .property("text", text)
            .build()
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind properties to widgets
        self.bind_property("icon", &imp.content.get(), "icon-name")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        self.bind_property("text", &imp.content.get(), "label")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }
}
