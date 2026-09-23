use gtk::glib;
use adw::subclass::prelude::*;

//------------------------------------------------------------------------------
// MODULE: MainMenuButton
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/main_menu_button.ui")]
    pub struct MainMenuButton {
        #[template_child]
        pub(super) popover: TemplateChild<gtk::PopoverMenu>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for MainMenuButton {
        const NAME: &'static str = "MainMenuButton";
        type Type = super::MainMenuButton;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MainMenuButton {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
        }
    }

    impl WidgetImpl for MainMenuButton {}
    impl BinImpl for MainMenuButton {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: MainMenuButton
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct MainMenuButton(ObjectSubclass<imp::MainMenuButton>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl MainMenuButton {
    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Add theme selector to popover
        let theme_selector = libpanel::ThemeSelector::new();
        theme_selector.set_action_name("win.set-color-scheme");

        imp.popover.add_child(&theme_selector, "theme_selector");
    }
}

impl Default for MainMenuButton {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
