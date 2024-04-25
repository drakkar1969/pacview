use std::cell::{Cell, RefCell};

use gtk::{glib, gio, gdk};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::RustClosure;
use glib::clone;

use crate::text_widget::{TextWidget, PropType};

//------------------------------------------------------------------------------
// MODULE: PropertyValue
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PropertyValue)]
    #[template(resource = "/com/github/PacView/ui/property_value.ui")]
    pub struct PropertyValue {
        #[template_child]
        pub image: TemplateChild<gtk::Image>,
        #[template_child]
        pub text_widget: TemplateChild<TextWidget>,

        #[template_child]
        pub popover_menu: TemplateChild<gtk::PopoverMenu>,

        #[property(get, set, builder(PropType::default()))]
        ptype: Cell<PropType>,
        #[property(get, set, nullable)]
        icon: RefCell<Option<String>>,
        #[property(get, set)]
        text: RefCell<String>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PropertyValue {
        const NAME: &'static str = "PropertyValue";
        type Type = super::PropertyValue;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_layout_manager_type::<gtk::BoxLayout>();
            klass.set_css_name("property-value");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PropertyValue {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_actions();
            obj.setup_shortcuts();
            obj.setup_controllers();
        }
    }

    impl WidgetImpl for PropertyValue {}
    impl BoxImpl for PropertyValue {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PropertyValue
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PropertyValue(ObjectSubclass<imp::PropertyValue>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PropertyValue {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(ptype: PropType, link_handler: RustClosure, select_handler: RustClosure) -> Self {
        let widget: Self = glib::Object::builder()
            .property("ptype", ptype)
            .build();

        let imp = widget.imp();

        imp.text_widget.connect_closure("link-activated", false, link_handler);
        imp.text_widget.connect_closure("selection-start", false, select_handler);

        widget
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind properties to widgets
        self.bind_property("ptype", &imp.text_widget.get(), "ptype")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        self.bind_property("text", &imp.text_widget.get(), "text")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        self.bind_property("icon", &imp.image.get(), "visible")
            .transform_to(|_, icon: Option<String>| Some(icon.is_some()))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        self.bind_property("icon", &imp.image.get(), "icon-name")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        let imp = self.imp();

        // Add select all action
        let select_action = gio::ActionEntry::builder("select-all")
            .activate(clone!(@weak imp => move |_, _, _| {
                imp.text_widget.select_all();
            }))
            .build();

        // Add copy action
        let copy_action = gio::ActionEntry::builder("copy")
            .activate(clone!(@weak self as widget, @weak imp => move |_, _, _| {
                if let Some(text) = imp.text_widget.selected_text() {
                    widget.clipboard().set_text(&text);
                }
            }))
            .build();

        // Add actions to text action group
        let text_group = gio::SimpleActionGroup::new();

        self.insert_action_group("text", Some(&text_group));

        text_group.add_action_entries([select_action, copy_action]);
    }

    //-----------------------------------
    // Setup shortcuts
    //-----------------------------------
    fn setup_shortcuts(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();

        // Add search start shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>A"),
            Some(gtk::NamedAction::new("text.select-all"))
        ));

        // Add search stop shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>C"),
            Some(gtk::NamedAction::new("text.copy"))
        ));

        // Add shortcut controller to window
        self.add_controller(controller);
    }

    //-----------------------------------
    // Setup controllers
    //-----------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        let click_gesture = gtk::GestureClick::new();
        click_gesture.set_button(0);

        click_gesture.connect_pressed(clone!(@weak self as widget, @weak imp => move |gesture, _, x, y| {
            // Focus widget on mouse press
            widget.grab_focus();

            if gesture.current_button() == gdk::BUTTON_SECONDARY {
                // Enable/disable copy action
                widget.action_set_enabled("text.copy", imp.text_widget.selected_text().is_some());

                // Show popover menu
                let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

                imp.popover_menu.set_pointing_to(Some(&rect));
                imp.popover_menu.popup();
            }
        }));

        self.add_controller(click_gesture);
    }
}
