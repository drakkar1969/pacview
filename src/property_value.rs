use std::cell::{Cell, RefCell};

use gtk::{glib, gdk, graphene};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use glib::RustClosure;

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
        pub(super) prop_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) text_widget: TemplateChild<TextWidget>,

        #[property(get, set, builder(PropType::default()))]
        ptype: Cell<PropType>,
        #[property(get, set)]
        label: RefCell<String>,
        #[property(get, set, nullable)]
        icon: RefCell<Option<String>>,
        #[property(get, set)]
        value: RefCell<String>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PropertyValue {
        const NAME: &'static str = "PropertyValue";
        type Type = super::PropertyValue;
        type ParentType = gtk::ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
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
            obj.setup_shortcuts();
            obj.setup_controllers();
        }

        //-----------------------------------
        // Dispose function
        //-----------------------------------
        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for PropertyValue {}
    impl ListBoxRowImpl for PropertyValue {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PropertyValue
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PropertyValue(ObjectSubclass<imp::PropertyValue>)
    @extends gtk::ListBoxRow, gtk::Widget,
    @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl PropertyValue {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(ptype: PropType) -> Self {
        glib::Object::builder()
            .property("ptype", ptype)
            .build()
    }

    //-----------------------------------
    // Set package link handler function
    //-----------------------------------
    pub fn set_pkg_link_handler(&self, handler: RustClosure) {
        let imp = self.imp();

        imp.text_widget.connect_closure("package-link", false, handler);
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind properties to widgets
        self.bind_property("label", &imp.prop_label.get(), "label")
            .sync_create()
            .build();

        self.bind_property("icon", &imp.image.get(), "visible")
            .transform_to(|_, icon: Option<String>| Some(icon.is_some()))
            .sync_create()
            .build();

        self.bind_property("icon", &imp.image.get(), "icon-name")
            .sync_create()
            .build();

        self.bind_property("ptype", &imp.text_widget.get(), "ptype")
            .sync_create()
            .build();

        self.bind_property("value", &imp.text_widget.get(), "text")
            .sync_create()
            .build();

        self.bind_property("has-focus", &imp.text_widget.get(), "is-focused")
            .sync_create()
            .build();
    }

    //-----------------------------------
    // Setup shortcuts
    //-----------------------------------
    fn setup_shortcuts(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();

        // Add select all shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>A"),
            Some(gtk::CallbackAction::new(|widget, _| {
                let property = widget
                    .downcast_ref::<PropertyValue>()
                    .expect("Could not downcast to 'PropertyValue'");

                property.imp().text_widget.activate_action("text.select-all", None).unwrap();

                glib::Propagation::Proceed
            }))
        ));

        // Add select none shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl><shift>A"),
            Some(gtk::CallbackAction::new(|widget, _| {
                let property = widget
                    .downcast_ref::<PropertyValue>()
                    .expect("Could not downcast to 'PropertyValue'");

                property.imp().text_widget.activate_action("text.select-none", None).unwrap();

                glib::Propagation::Proceed
            }))
        ));

        // Add copy shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>C"),
            Some(gtk::CallbackAction::new(|widget, _| {
                let property = widget
                    .downcast_ref::<PropertyValue>()
                    .expect("Could not downcast to 'PropertyValue'");

                property.imp().text_widget.activate_action("text.copy", None).unwrap();

                glib::Propagation::Proceed
            }))
        ));

        // Add shortcut controller to property
        self.add_controller(controller);
    }

    //-----------------------------------
    // Setup controllers
    //-----------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        // Add mouse click controller
        let click_gesture = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();

        click_gesture.connect_pressed(clone!(
            #[weak(rename_to = property)] self,
            move |_, _, _, _| {
                property.grab_focus();
            }
        ));

        self.add_controller(click_gesture);

        // Add popup menu controller
        let popup_gesture = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();

        popup_gesture.connect_pressed(clone!(
            #[weak(rename_to = property)] self,
            #[weak] imp,
            move |_, _, x, y| {
                if let Some(point) = property.compute_point(&imp.text_widget.get(), &graphene::Point::new(x as f32, y as f32)) {
                    imp.text_widget.popup_menu(point.x() as f64, point.y() as f64);
                }
            }
        ));

        self.add_controller(popup_gesture);

        // Add key press controller
        let key_controller = gtk::EventControllerKey::new();

        key_controller.connect_key_pressed(clone!(
            #[weak] imp,
            #[upgrade_or] glib::Propagation::Proceed,
            move |_, key, _, state| {
                if state == gdk::ModifierType::empty() {
                    if key == gdk::Key::Left {
                        imp.text_widget.key_left();

                        return glib::Propagation::Stop
                    }

                    if key == gdk::Key::Right {
                        imp.text_widget.key_right();

                        return glib::Propagation::Stop
                    }

                    if key == gdk::Key::Return || key == gdk::Key::KP_Enter {
                        imp.text_widget.key_return();

                        return glib::Propagation::Stop
                    }
                }

                glib::Propagation::Proceed
            }
        ));

        self.add_controller(key_controller);
    }
}
